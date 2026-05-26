import { browser } from "$app/environment";
import {
  fetchSubscription,
  pingTcp,
  flagFor,
  stripLeadingFlag,
  formatBytes,
  formatExpires,
  type ImportResult,
  type VlessServer,
} from "$lib/api";

export interface ServerEntry {
  id: string;
  flag: string;
  name: string;
  transport: string;
  pingMs: number | null;
  pinging: boolean;
  raw: VlessServer;
}

export interface Subscription {
  id: string;
  name: string;
  /** Free-text description sourced from a leading `# …` comment in the
   *  subscription body. null when the server doesn't include one. */
  description: string | null;
  url: string;
  importedAt: string; // ISO
  /** Server-advertised refresh interval (hours). null when not sent. */
  updateIntervalHours: number | null;
  /** Bytes used (upload + download). */
  usedBytes: number;
  /** Total quota in bytes; 0 = unlimited. */
  totalBytes: number;
  /** Unix seconds, or null when no expiry was sent. */
  expiresAtUnix: number | null;
  /** Telegram/support contact (Support-Url) — paper-plane icon when it's a
   *  t.me link, which for our own service is the bot. */
  supportUrl: string | null;
  /** Provider website (Profile-Web-Page-Url) — shown as an info icon. */
  webPageUrl: string | null;
  servers: ServerEntry[];
  collapsed: boolean;
  /** True while refresh() is in flight. Not persisted. */
  refreshing?: boolean;
}

interface Persisted {
  subs: Subscription[];
  selectedServerId: string | null;
}

const KEY = "aegisvpn.subs";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** Earlier versions used a deterministic `host:port#uuid8` for ServerEntry.id.
 *  When two subscriptions advertised the same endpoint, those IDs collided
 *  and broke `{#each}`'s keyed reconciliation. New entries use random UUIDs,
 *  so we transparently regenerate any old-format IDs the first time we load.
 */
function migrateIds(subs: Subscription[]): { subs: Subscription[]; remapped: Record<string, string> } {
  const remapped: Record<string, string> = {};
  for (const sub of subs) {
    for (const srv of sub.servers) {
      if (!srv.id || !UUID_RE.test(srv.id)) {
        const fresh = crypto.randomUUID();
        remapped[srv.id ?? ""] = fresh;
        srv.id = fresh;
      }
      if (typeof srv.pinging !== "boolean") srv.pinging = false;
      // Drop the leading flag emoji from older labels stored before the
      // flag was rendered separately.
      srv.name = stripLeadingFlag(srv.name);
      // Re-derive the flag from the original label so entries imported before
      // we preferred the label's own flag emoji pick up the correct one.
      if (srv.raw?.label) srv.flag = flagFor(srv.raw.label);
    }
    if (sub.description === undefined) sub.description = null;
    if (sub.webPageUrl === undefined) sub.webPageUrl = null;
    if (sub.refreshing) sub.refreshing = false;
  }
  return { subs, remapped };
}

function load(): Persisted {
  if (!browser) return { subs: [], selectedServerId: null };
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { subs: [], selectedServerId: null };
    const parsed = JSON.parse(raw) as Partial<Persisted>;
    const rawSubs = Array.isArray(parsed.subs) ? parsed.subs : [];
    const { subs, remapped } = migrateIds(rawSubs);
    let selected: string | null =
      typeof parsed.selectedServerId === "string" ? parsed.selectedServerId : null;
    if (selected && remapped[selected]) selected = remapped[selected];
    // If the persisted selection points at an id we no longer have, drop it.
    if (selected && !subs.some((s) => s.servers.some((sv) => sv.id === selected))) {
      selected = null;
    }
    return { subs, selectedServerId: selected };
  } catch {
    return { subs: [], selectedServerId: null };
  }
}

const PROTOCOL_LABELS: Record<string, string> = {
  vless: "VLESS",
  trojan: "Trojan",
  shadowsocks: "Shadowsocks",
  vmess: "VMess",
};

function transportSummary(s: VlessServer): string {
  const proto = PROTOCOL_LABELS[s.protocol] ?? s.protocol.toUpperCase();
  const parts = [proto, s.transport.toUpperCase()];
  if (s.security && s.security !== "none") parts.push(s.security.toUpperCase());
  return parts.join(" / ");
}

function toServerEntry(s: VlessServer): ServerEntry {
  return {
    // Random id avoids collisions when two subscriptions advertise the same
    // host:port endpoint (otherwise Svelte's keyed {#each} blows up the
    // second render).
    id: crypto.randomUUID(),
    flag: flagFor(s.label),
    name: stripLeadingFlag(s.label),
    transport: transportSummary(s),
    pingMs: null,
    pinging: false,
    raw: s,
  };
}

function deriveSubName(result: ImportResult, url: string): string {
  if (result.meta.title) return result.meta.title;
  for (const s of result.servers) {
    const left = s.label.split(/[|·•—-]/)[0]?.trim();
    if (left && left.length > 1 && left.length < 24) return left;
  }
  try {
    return new URL(url).hostname;
  } catch {
    return "Subscription";
  }
}

// Hydrate from localStorage once when the module first loads, before the
// SubsStore class fields are evaluated. (Referencing `this` from inside a
// `$state(...)` field initialiser blew up Svelte 5's compiled output.)
const _initialSubs = load();

class SubsStore {
  list = $state<Subscription[]>(_initialSubs.subs);
  selectedServerId = $state<string | null>(_initialSubs.selectedServerId);
  importing = $state(false);

  private persist(): void {
    if (!browser) return;
    localStorage.setItem(
      KEY,
      JSON.stringify({
        subs: this.list,
        selectedServerId: this.selectedServerId,
      }),
    );
  }

  selectServer(id: string): void {
    this.selectedServerId = id;
    this.persist();
  }

  /** Parsed server for the current selection, or null if nothing is selected. */
  selectedServerRaw(): VlessServer | null {
    const id = this.selectedServerId;
    if (!id) return null;
    for (const sub of this.list) {
      const srv = sub.servers.find((s) => s.id === id);
      if (srv) return srv.raw;
    }
    return null;
  }

  toggleCollapse(subId: string): void {
    const s = this.list.find((x) => x.id === subId);
    if (s) {
      s.collapsed = !s.collapsed;
      this.persist();
    }
  }

  collapseAll(): void {
    for (const s of this.list) s.collapsed = true;
    this.persist();
  }

  expandAll(): void {
    for (const s of this.list) s.collapsed = false;
    this.persist();
  }

  remove(subId: string): void {
    this.list = this.list.filter((s) => s.id !== subId);
    if (
      this.selectedServerId &&
      !this.list.some((s) => s.servers.some((sv) => sv.id === this.selectedServerId))
    ) {
      this.selectedServerId = null;
    }
    this.persist();
  }

  trafficText(sub: Subscription): string {
    const used = formatBytes(sub.usedBytes);
    const total = sub.totalBytes > 0 ? formatBytes(sub.totalBytes) : "∞";
    return `${used}/${total}`;
  }

  expiresText(sub: Subscription): string | null {
    return formatExpires(sub.expiresAtUnix);
  }

  async importFromUrl(url: string): Promise<void> {
    const trimmed = url.trim();
    if (!trimmed) throw new Error("empty url");
    this.importing = true;
    try {
      const result = await fetchSubscription(trimmed);
      if (result.servers.length === 0) {
        throw new Error("no servers found in this subscription");
      }
      const servers = result.servers.map(toServerEntry);
      const totalBytes = result.meta.total_bytes ?? 0;
      const usedBytes =
        (result.meta.upload_bytes ?? 0) + (result.meta.download_bytes ?? 0);

      const sub: Subscription = {
        id: crypto.randomUUID(),
        name: deriveSubName(result, trimmed),
        description: result.description,
        url: trimmed,
        importedAt: new Date().toISOString(),
        updateIntervalHours: result.meta.update_interval_hours ?? null,
        usedBytes,
        totalBytes,
        expiresAtUnix: result.meta.expires_at_unix,
        supportUrl: result.meta.support_url,
        webPageUrl: result.meta.web_page_url,
        servers,
        collapsed: false,
      };
      this.list = [...this.list, sub];
      if (!this.selectedServerId && servers.length > 0) {
        this.selectedServerId = servers[0].id;
      }
      this.persist();
      // Ping in the background; don't block the import dialog on it.
      void this.pingAll(sub.id);
    } finally {
      this.importing = false;
    }
  }

  async refresh(subId: string): Promise<void> {
    const idx = this.list.findIndex((s) => s.id === subId);
    if (idx < 0) return;
    const sub = this.list[idx];
    // mark this sub as refreshing for the UI spinner
    this.list = this.list.map((s) =>
      s.id === subId ? { ...s, refreshing: true } : s,
    );
    try {
      const result = await fetchSubscription(sub.url);
      if (result.servers.length === 0) {
        this.list = this.list.map((s) =>
          s.id === subId ? { ...s, refreshing: false } : s,
        );
        return;
      }
      const totalBytes = result.meta.total_bytes ?? sub.totalBytes;
      const usedBytes =
        (result.meta.upload_bytes ?? 0) + (result.meta.download_bytes ?? 0);
      this.list = this.list.map((s) =>
        s.id === subId
          ? {
              ...s,
              name: result.meta.title ?? s.name,
              description: result.description ?? s.description,
              servers: result.servers.map(toServerEntry),
              updateIntervalHours:
                result.meta.update_interval_hours ?? s.updateIntervalHours,
              usedBytes,
              totalBytes,
              expiresAtUnix: result.meta.expires_at_unix ?? s.expiresAtUnix,
              supportUrl: result.meta.support_url,
              webPageUrl: result.meta.web_page_url,
              importedAt: new Date().toISOString(),
              refreshing: false,
            }
          : s,
      );
      this.persist();
      void this.pingAll(subId);
    } catch (e) {
      console.error("refresh failed:", e);
      this.list = this.list.map((s) =>
        s.id === subId ? { ...s, refreshing: false } : s,
      );
    }
  }

  rename(subId: string, newName: string): void {
    const trimmed = newName.trim();
    if (!trimmed) return;
    this.list = this.list.map((s) =>
      s.id === subId ? { ...s, name: trimmed } : s,
    );
    this.persist();
  }

  /** Re-ping every server in a subscription. Updates `pingMs` in place.
   *
   *  Plain TCP-connect RTT — measures the real wire latency to host:port,
   *  same number ICMP `ping` would show. Cheap, accurate, runs all servers
   *  in parallel. (It deliberately doesn't open a TLS / Reality handshake;
   *  that would inflate the number to multiples of the real latency, which
   *  is the trap clients like Happ fall into.) */
  async pingAll(subId: string): Promise<void> {
    const sub = this.list.find((s) => s.id === subId);
    if (!sub) return;
    // mark all as pinging upfront so the UI dims their values
    this.list = this.list.map((s) =>
      s.id === subId
        ? { ...s, servers: s.servers.map((sv) => ({ ...sv, pinging: true })) }
        : s,
    );

    await Promise.all(
      sub.servers.map(async (sv) => {
        let pingMs: number | null = null;
        try {
          pingMs = await pingTcp(sv.raw.host, sv.raw.port);
        } catch {
          pingMs = null;
        }
        this.list = this.list.map((s) =>
          s.id === subId
            ? {
                ...s,
                servers: s.servers.map((x) =>
                  x.id === sv.id ? { ...x, pingMs, pinging: false } : x,
                ),
              }
            : s,
        );
      }),
    );
    this.persist();
  }
}

export const subs = new SubsStore();

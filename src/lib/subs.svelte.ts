import { browser } from "$app/environment";
import {
  fetchSubscription,
  flagFor,
  stripLeadingFlag,
  formatBytes,
  formatExpires,
  tcpPingHost,
  type ImportResult,
  type VlessServer,
} from "$lib/api";

/** Ping result for a server entry. `null` = unknown / not yet measured,
 *  `"pinging"` = probe in flight, `"timeout"` = host unreachable / timed out,
 *  number = RTT in milliseconds. */
export type PingState = number | "pinging" | "timeout";

export interface ServerEntry {
  id: string;
  flag: string;
  name: string;
  transport: string;
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
      // Drop any legacy ping fields that older versions persisted on the
      // entry — pinging is gone from the UI for now.
      delete (srv as unknown as Record<string, unknown>).pingMs;
      delete (srv as unknown as Record<string, unknown>).pinging;
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
    // No quota (total=0 = unlimited) → show just the bare used figure, not
    // "X/∞" — the infinity denominator is noise when there's no cap.
    if (sub.totalBytes > 0) return `${used}/${formatBytes(sub.totalBytes)}`;
    return used;
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
      // Kick a ping pass for the new servers so the UI doesn't sit on "–"
      // until the user opens it. Intentionally not awaited.
      void Promise.all(servers.map((srv) => this.pingServer(srv)));
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
      const freshServers = result.servers.map(toServerEntry);
      this.list = this.list.map((s) =>
        s.id === subId
          ? {
              ...s,
              name: result.meta.title ?? s.name,
              description: result.description ?? s.description,
              servers: freshServers,
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
      // The old server IDs were just dropped (new ones are random), so the
      // old `pings` entries are dead weight — refresh them.
      void Promise.all(freshServers.map((srv) => this.pingServer(srv)));
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

  // Ephemeral per-server ping state. Not persisted — we re-measure on app
  // start and on subscription refresh, mirroring what Happ does on open.
  pings = $state<Record<string, PingState>>({});

  /** Probe one server. Updates `pings[id]` in place; never throws. */
  async pingServer(srv: ServerEntry): Promise<void> {
    this.pings = { ...this.pings, [srv.id]: "pinging" };
    try {
      const rtt = await tcpPingHost(srv.raw.host, srv.raw.port, 2500);
      this.pings = { ...this.pings, [srv.id]: rtt };
    } catch {
      this.pings = { ...this.pings, [srv.id]: "timeout" };
    }
  }

  /** Probe every server across every subscription in parallel. Safe to call
   *  while one is already in flight; the in-flight ones just get overwritten
   *  with fresh values. */
  async pingAll(): Promise<void> {
    const all: ServerEntry[] = this.list.flatMap((s) => s.servers);
    await Promise.all(all.map((srv) => this.pingServer(srv)));
  }

  /** Probe every server inside a single subscription in parallel. Used by the
   *  per-subscription ping button. */
  async pingSub(subId: string): Promise<void> {
    const sub = this.list.find((s) => s.id === subId);
    if (!sub) return;
    await Promise.all(sub.servers.map((srv) => this.pingServer(srv)));
  }

  /** True iff at least one server in this subscription has an in-flight probe. */
  isSubPinging(subId: string): boolean {
    const sub = this.list.find((s) => s.id === subId);
    if (!sub) return false;
    return sub.servers.some((srv) => this.pings[srv.id] === "pinging");
  }
}

export const subs = new SubsStore();

import { browser } from "$app/environment";
import {
  fetchSubscription,
  pingTcp,
  guessFlag,
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
  url: string;
  importedAt: string; // ISO
  updateIntervalHours: number;
  /** Bytes used (upload + download). */
  usedBytes: number;
  /** Total quota in bytes; 0 = unlimited. */
  totalBytes: number;
  /** Unix seconds, or null when no expiry was sent. */
  expiresAtUnix: number | null;
  supportUrl: string | null;
  servers: ServerEntry[];
  collapsed: boolean;
}

interface Persisted {
  subs: Subscription[];
  selectedServerId: string | null;
}

const KEY = "aegisvpn.subs";

function load(): Persisted {
  if (!browser) return { subs: [], selectedServerId: null };
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { subs: [], selectedServerId: null };
    const parsed = JSON.parse(raw) as Partial<Persisted>;
    return {
      subs: Array.isArray(parsed.subs) ? parsed.subs : [],
      selectedServerId: typeof parsed.selectedServerId === "string"
        ? parsed.selectedServerId
        : null,
    };
  } catch {
    return { subs: [], selectedServerId: null };
  }
}

function transportSummary(s: VlessServer): string {
  return `VLESS / ${s.transport.toUpperCase()} / ${s.security.toUpperCase()}`;
}

function toServerEntry(s: VlessServer): ServerEntry {
  return {
    id: `${s.host}:${s.port}#${s.uuid.slice(0, 8)}`,
    flag: guessFlag(s.label),
    name: s.label,
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

class SubsStore {
  list = $state<Subscription[]>([]);
  selectedServerId = $state<string | null>(null);
  importing = $state(false);

  init(): void {
    const p = load();
    this.list = p.subs;
    this.selectedServerId = p.selectedServerId;
  }

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
        url: trimmed,
        importedAt: new Date().toISOString(),
        updateIntervalHours: result.meta.update_interval_hours ?? 12,
        usedBytes,
        totalBytes,
        expiresAtUnix: result.meta.expires_at_unix,
        supportUrl: result.meta.support_url,
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
    try {
      const result = await fetchSubscription(sub.url);
      if (result.servers.length === 0) return;
      const totalBytes = result.meta.total_bytes ?? sub.totalBytes;
      const usedBytes =
        (result.meta.upload_bytes ?? 0) + (result.meta.download_bytes ?? 0);
      this.list = this.list.map((s) =>
        s.id === subId
          ? {
              ...s,
              name: result.meta.title ?? s.name,
              servers: result.servers.map(toServerEntry),
              updateIntervalHours:
                result.meta.update_interval_hours ?? s.updateIntervalHours,
              usedBytes,
              totalBytes,
              expiresAtUnix: result.meta.expires_at_unix ?? s.expiresAtUnix,
              supportUrl: result.meta.support_url ?? s.supportUrl,
              importedAt: new Date().toISOString(),
            }
          : s,
      );
      this.persist();
      void this.pingAll(subId);
    } catch (e) {
      console.error("refresh failed:", e);
    }
  }

  /** Re-ping every server in a subscription. Updates `pingMs` in place. */
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

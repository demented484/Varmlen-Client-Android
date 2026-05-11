import { browser } from "$app/environment";
import { fetchSubscription, guessFlag, type VlessServer } from "$lib/api";

export interface ServerEntry {
  id: string;
  flag: string;
  name: string;
  transport: string;
  pingMs: number | null;
  raw: VlessServer;
}

export interface Subscription {
  id: string;
  name: string;
  url: string;
  importedAt: string; // ISO
  updateIntervalHours: number;
  trafficUsed: string;
  trafficTotal: string;
  expiresAt: string | null;
  telegramUrl: string | null;
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
    raw: s,
  };
}

function deriveSubName(servers: VlessServer[], url: string): string {
  // Try the most common label like "AegisVPN | Finland" → "AegisVPN".
  for (const s of servers) {
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

  async importFromUrl(url: string): Promise<void> {
    const trimmed = url.trim();
    if (!trimmed) throw new Error("empty url");
    this.importing = true;
    try {
      const parsed = await fetchSubscription(trimmed);
      if (parsed.length === 0) {
        throw new Error("no servers found in this subscription");
      }
      const servers = parsed.map(toServerEntry);
      const sub: Subscription = {
        id: crypto.randomUUID(),
        name: deriveSubName(parsed, trimmed),
        url: trimmed,
        importedAt: new Date().toISOString(),
        updateIntervalHours: 12,
        trafficUsed: "0B",
        trafficTotal: "∞",
        expiresAt: null,
        telegramUrl: null,
        servers,
        collapsed: false,
      };
      this.list = [...this.list, sub];
      if (!this.selectedServerId && servers.length > 0) {
        this.selectedServerId = servers[0].id;
      }
      this.persist();
    } finally {
      this.importing = false;
    }
  }

  async refresh(subId: string): Promise<void> {
    const idx = this.list.findIndex((s) => s.id === subId);
    if (idx < 0) return;
    const sub = this.list[idx];
    try {
      const parsed = await fetchSubscription(sub.url);
      if (parsed.length === 0) return;
      this.list = this.list.map((s) =>
        s.id === subId
          ? { ...s, servers: parsed.map(toServerEntry), importedAt: new Date().toISOString() }
          : s,
      );
      this.persist();
    } catch (e) {
      console.error("refresh failed:", e);
    }
  }

  pingAll(_subId: string): Promise<void> {
    // TODO: invoke('ping_all_servers', { id })
    return Promise.resolve();
  }
}

export const subs = new SubsStore();

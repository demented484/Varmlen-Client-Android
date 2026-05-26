import { browser } from "$app/environment";

export type Mode = "selective" | "general";

export interface AppEntry {
  /** Process name on Linux/Windows or package on Android. */
  id: string;
  /** Display name. */
  name: string;
  /** Emoji or short text placeholder until we resolve real icons. */
  icon: string;
  enabled: boolean;
}

export interface SiteEntry {
  id: string;
  pattern: string;
  enabled: boolean;
}

/** A single mode applies to BOTH apps and sites so the routing is predictable:
 *  selective = whitelist (only listed apps/sites go through the VPN),
 *  general   = blacklist (all traffic goes through the VPN, listed entries are
 *              exceptions and stay direct).
 *
 *  The previous design exposed independent modes per list, which silently
 *  combined into surprising behavior — e.g. "apps selective, sites general"
 *  defaulted to proxy, so selective apps didn't actually whitelist anything. */
interface Persisted {
  mode: Mode;
  apps: AppEntry[];
  sites: SiteEntry[];
}

const KEY = "aegisvpn.split";

function defaults(): Persisted {
  // Default to "general": VPN carries everything, exceptions are direct. With
  // no exceptions this means "everything via VPN", which is what users expect
  // right after enabling the toggle.
  return { mode: "general", apps: [], sites: [] };
}

function load(): Persisted {
  if (!browser) return defaults();
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return defaults();
    const parsed = JSON.parse(raw) as Record<string, unknown>;

    // New shape — used as-is.
    if (typeof parsed.mode === "string" && Array.isArray(parsed.apps) && Array.isArray(parsed.sites)) {
      const mode: Mode = parsed.mode === "selective" ? "selective" : "general";
      return { mode, apps: parsed.apps as AppEntry[], sites: parsed.sites as SiteEntry[] };
    }

    // Legacy shape: separate appsMode/sitesMode + ByMode-bucketed lists.
    // Pick "selective" as the unified mode iff either old mode was selective
    // (that's what the user wanted to express); take the items out of the
    // corresponding bucket. This loses items in the OTHER bucket; the old
    // design didn't really let you use both anyway.
    const oldApps = parsed.appsMode === "selective" ? "selective" : "general";
    const oldSites = parsed.sitesMode === "selective" ? "selective" : "general";
    const mode: Mode = oldApps === "selective" || oldSites === "selective" ? "selective" : "general";

    const apps: AppEntry[] = (() => {
      const o = parsed.apps;
      if (Array.isArray(o)) return o as AppEntry[];
      if (o && typeof o === "object") {
        const bucket = (o as Record<string, AppEntry[]>)[mode];
        if (Array.isArray(bucket)) return bucket;
      }
      return [];
    })();
    const sites: SiteEntry[] = (() => {
      const o = parsed.sites;
      if (Array.isArray(o)) return o as SiteEntry[];
      if (o && typeof o === "object") {
        const bucket = (o as Record<string, SiteEntry[]>)[mode];
        if (Array.isArray(bucket)) return bucket;
      }
      return [];
    })();
    return { mode, apps, sites };
  } catch {
    return defaults();
  }
}

const _initialSplit = load();

class SplitStore {
  mode = $state<Mode>(_initialSplit.mode);
  apps = $state<AppEntry[]>(_initialSplit.apps);
  sites = $state<SiteEntry[]>(_initialSplit.sites);

  private persist(): void {
    if (!browser) return;
    const payload: Persisted = { mode: this.mode, apps: this.apps, sites: this.sites };
    localStorage.setItem(KEY, JSON.stringify(payload));
  }

  setMode(m: Mode): void {
    this.mode = m;
    this.persist();
  }

  toggleApp(id: string): void {
    this.apps = this.apps.map((a) => (a.id === id ? { ...a, enabled: !a.enabled } : a));
    this.persist();
  }

  addApp(app: Omit<AppEntry, "enabled">): void {
    if (this.apps.some((a) => a.id === app.id)) return;
    this.apps = [...this.apps, { ...app, enabled: true }];
    this.persist();
  }

  removeApp(id: string): void {
    this.apps = this.apps.filter((a) => a.id !== id);
    this.persist();
  }

  addSite(pattern: string): void {
    const v = pattern.trim();
    if (!v) return;
    if (this.sites.some((s) => s.pattern === v)) return;
    this.sites = [...this.sites, { id: crypto.randomUUID(), pattern: v, enabled: true }];
    this.persist();
  }

  toggleSite(id: string): void {
    this.sites = this.sites.map((s) => (s.id === id ? { ...s, enabled: !s.enabled } : s));
    this.persist();
  }

  removeSite(id: string): void {
    this.sites = this.sites.filter((s) => s.id !== id);
    this.persist();
  }
}

export const split = new SplitStore();

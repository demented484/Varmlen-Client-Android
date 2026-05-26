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

/** Lists are kept independently per mode: the selective and general lists are
 *  unrelated, so switching the mode swaps to that mode's own set of entries. */
type ByMode<T> = Record<Mode, T[]>;

interface Persisted {
  appsMode: Mode;
  sitesMode: Mode;
  apps: ByMode<AppEntry>;
  sites: ByMode<SiteEntry>;
}

const KEY = "aegisvpn.split";

function emptyApps(): ByMode<AppEntry> {
  return { selective: [], general: [] };
}
function emptySites(): ByMode<SiteEntry> {
  return { selective: [], general: [] };
}

function defaults(): Persisted {
  // Default to "general": the VPN carries everything, and split entries are
  // exceptions that stay direct. With no entries this means "all traffic via
  // VPN", which is what a user expects right after connecting.
  return {
    appsMode: "general",
    sitesMode: "general",
    apps: emptyApps(),
    sites: emptySites(),
  };
}

function load(): Persisted {
  if (!browser) return defaults();
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return defaults();
    const parsed = JSON.parse(raw) as Record<string, unknown> & { mode?: Mode };

    const legacy: Mode = parsed.mode === "general" ? "general" : "selective";
    const appsMode: Mode =
      parsed.appsMode === "general" ? "general" : parsed.appsMode === "selective" ? "selective" : legacy;
    const sitesMode: Mode =
      parsed.sitesMode === "general" ? "general" : parsed.sitesMode === "selective" ? "selective" : legacy;

    // apps / sites may be the new per-mode object or a legacy flat array; a
    // legacy array is migrated into whatever mode was active at save time.
    const apps = emptyApps();
    const sites = emptySites();
    if (Array.isArray(parsed.apps)) {
      apps[appsMode] = parsed.apps as AppEntry[];
    } else if (parsed.apps && typeof parsed.apps === "object") {
      const o = parsed.apps as Partial<ByMode<AppEntry>>;
      if (Array.isArray(o.selective)) apps.selective = o.selective;
      if (Array.isArray(o.general)) apps.general = o.general;
    }
    if (Array.isArray(parsed.sites)) {
      sites[sitesMode] = parsed.sites as SiteEntry[];
    } else if (parsed.sites && typeof parsed.sites === "object") {
      const o = parsed.sites as Partial<ByMode<SiteEntry>>;
      if (Array.isArray(o.selective)) sites.selective = o.selective;
      if (Array.isArray(o.general)) sites.general = o.general;
    }

    return { appsMode, sitesMode, apps, sites };
  } catch {
    return defaults();
  }
}

const _initialSplit = load();

class SplitStore {
  appsMode = $state<Mode>(_initialSplit.appsMode);
  sitesMode = $state<Mode>(_initialSplit.sitesMode);
  apps = $state<ByMode<AppEntry>>(_initialSplit.apps);
  sites = $state<ByMode<SiteEntry>>(_initialSplit.sites);

  /** Apps for the currently selected apps-mode. */
  get currentApps(): AppEntry[] {
    return this.apps[this.appsMode];
  }
  /** Sites for the currently selected sites-mode. */
  get currentSites(): SiteEntry[] {
    return this.sites[this.sitesMode];
  }

  private persist(): void {
    if (!browser) return;
    const payload: Persisted = {
      appsMode: this.appsMode,
      sitesMode: this.sitesMode,
      apps: this.apps,
      sites: this.sites,
    };
    localStorage.setItem(KEY, JSON.stringify(payload));
  }

  setAppsMode(m: Mode): void {
    this.appsMode = m;
    this.persist();
  }

  setSitesMode(m: Mode): void {
    this.sitesMode = m;
    this.persist();
  }

  private setApps(list: AppEntry[]): void {
    this.apps = { ...this.apps, [this.appsMode]: list };
    this.persist();
  }

  private setSites(list: SiteEntry[]): void {
    this.sites = { ...this.sites, [this.sitesMode]: list };
    this.persist();
  }

  toggleApp(id: string): void {
    this.setApps(this.currentApps.map((a) => (a.id === id ? { ...a, enabled: !a.enabled } : a)));
  }

  addApp(app: Omit<AppEntry, "enabled">): void {
    if (this.currentApps.some((a) => a.id === app.id)) return;
    this.setApps([...this.currentApps, { ...app, enabled: true }]);
  }

  removeApp(id: string): void {
    this.setApps(this.currentApps.filter((a) => a.id !== id));
  }

  addSite(pattern: string): void {
    const v = pattern.trim();
    if (!v) return;
    if (this.currentSites.some((s) => s.pattern === v)) return;
    this.setSites([...this.currentSites, { id: crypto.randomUUID(), pattern: v, enabled: true }]);
  }

  toggleSite(id: string): void {
    this.setSites(this.currentSites.map((s) => (s.id === id ? { ...s, enabled: !s.enabled } : s)));
  }

  removeSite(id: string): void {
    this.setSites(this.currentSites.filter((s) => s.id !== id));
  }
}

export const split = new SplitStore();

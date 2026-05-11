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

interface Persisted {
  mode: Mode;
  apps: AppEntry[];
  sites: SiteEntry[];
}

const KEY = "aegisvpn.split";
const DEFAULTS: Persisted = {
  mode: "selective",
  apps: [],
  sites: [],
};

function load(): Persisted {
  if (!browser) return DEFAULTS;
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return DEFAULTS;
    const parsed = JSON.parse(raw) as Partial<Persisted>;
    return {
      mode: parsed.mode === "general" ? "general" : "selective",
      apps: Array.isArray(parsed.apps) ? parsed.apps : [],
      sites: Array.isArray(parsed.sites) ? parsed.sites : [],
    };
  } catch {
    return DEFAULTS;
  }
}

class SplitStore {
  mode = $state<Mode>(DEFAULTS.mode);
  apps = $state<AppEntry[]>([]);
  sites = $state<SiteEntry[]>([]);

  init(): void {
    const p = load();
    this.mode = p.mode;
    this.apps = p.apps;
    this.sites = p.sites;
  }

  private persist(): void {
    if (!browser) return;
    const payload: Persisted = {
      mode: this.mode,
      apps: this.apps,
      sites: this.sites,
    };
    localStorage.setItem(KEY, JSON.stringify(payload));
  }

  setMode(m: Mode): void {
    this.mode = m;
    this.persist();
  }

  toggleApp(id: string): void {
    this.apps = this.apps.map((a) =>
      a.id === id ? { ...a, enabled: !a.enabled } : a,
    );
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
    this.sites = [
      ...this.sites,
      { id: crypto.randomUUID(), pattern: v, enabled: true },
    ];
    this.persist();
  }

  toggleSite(id: string): void {
    this.sites = this.sites.map((s) =>
      s.id === id ? { ...s, enabled: !s.enabled } : s,
    );
    this.persist();
  }

  removeSite(id: string): void {
    this.sites = this.sites.filter((s) => s.id !== id);
    this.persist();
  }
}

export const split = new SplitStore();

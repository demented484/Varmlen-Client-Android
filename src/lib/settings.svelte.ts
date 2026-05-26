import { browser } from "$app/environment";

export type VpnMode = "tun" | "proxy";

interface Persisted {
  vpnMode: VpnMode;
  killswitch: boolean;
  allowLan: boolean;
}

const KEY = "aegisvpn.settings";
const DEFAULTS: Persisted = {
  vpnMode: "tun",
  killswitch: true,
  allowLan: true,
};

function load(): Persisted {
  if (!browser) return DEFAULTS;
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return DEFAULTS;
    const parsed = JSON.parse(raw) as Partial<Persisted>;
    return {
      vpnMode: parsed.vpnMode === "proxy" ? "proxy" : "tun",
      killswitch: parsed.killswitch ?? DEFAULTS.killswitch,
      allowLan: parsed.allowLan ?? DEFAULTS.allowLan,
    };
  } catch {
    return DEFAULTS;
  }
}

const _initialSettings = load();

class SettingsStore {
  vpnMode = $state<VpnMode>(_initialSettings.vpnMode);
  killswitch = $state(_initialSettings.killswitch);
  allowLan = $state(_initialSettings.allowLan);

  private persist(): void {
    if (!browser) return;
    localStorage.setItem(
      KEY,
      JSON.stringify({
        vpnMode: this.vpnMode,
        killswitch: this.killswitch,
        allowLan: this.allowLan,
      }),
    );
  }

  setVpnMode(v: VpnMode): void { this.vpnMode = v; this.persist(); }
  setKillswitch(v: boolean): void { this.killswitch = v; this.persist(); }
  setAllowLan(v: boolean): void { this.allowLan = v; this.persist(); }
}

export const settings = new SettingsStore();

import { browser } from "$app/environment";

export type VpnMode = "tun" | "proxy";
/** How server latency is measured. `tcp` = raw TCP connect to the endpoint
 *  (bypasses the tunnel, works disconnected). `proxy` = an HTTP GET routed
 *  through a throwaway xray per server (via-proxy latency). */
export type PingMethod = "tcp" | "proxy";

interface Persisted {
  vpnMode: VpnMode;
  killswitch: boolean;
  allowLan: boolean;
  pingMethod: PingMethod;
  /** Closing the window hides to the tray (true) vs fully quits (false). */
  closeToTray: boolean;
}

const KEY = "varmlen.settings";
const DEFAULTS: Persisted = {
  vpnMode: "tun",
  killswitch: true,
  allowLan: true,
  pingMethod: "tcp",
  closeToTray: true,
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
      pingMethod: parsed.pingMethod === "proxy" ? "proxy" : "tcp",
      closeToTray: parsed.closeToTray ?? DEFAULTS.closeToTray,
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
  pingMethod = $state<PingMethod>(_initialSettings.pingMethod);
  closeToTray = $state(_initialSettings.closeToTray);

  private persist(): void {
    if (!browser) return;
    localStorage.setItem(
      KEY,
      JSON.stringify({
        vpnMode: this.vpnMode,
        killswitch: this.killswitch,
        allowLan: this.allowLan,
        pingMethod: this.pingMethod,
        closeToTray: this.closeToTray,
      }),
    );
  }

  setVpnMode(v: VpnMode): void { this.vpnMode = v; this.persist(); }
  setKillswitch(v: boolean): void { this.killswitch = v; this.persist(); }
  setAllowLan(v: boolean): void { this.allowLan = v; this.persist(); }
  setPingMethod(v: PingMethod): void { this.pingMethod = v; this.persist(); }
  setCloseToTray(v: boolean): void { this.closeToTray = v; this.persist(); }
}

export const settings = new SettingsStore();

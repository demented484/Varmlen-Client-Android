import { vpnConnect, vpnDisconnect, vpnStatus, type SplitInput } from "$lib/api";
import { subs } from "$lib/subs.svelte";
import { split } from "$lib/split.svelte";
import { settings } from "$lib/settings.svelte";
import { t } from "$lib/i18n.svelte";

export type Status = "disconnected" | "connecting" | "connected";

function msg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** Current split-tunnel selection (enabled entries for the active mode). */
function splitInput(): SplitInput {
  return {
    mode: split.mode,
    apps: split.apps.filter((a) => a.enabled).map((a) => a.id),
    sites: split.sites.filter((s) => s.enabled).map((s) => s.pattern),
  };
}

class ConnStore {
  status = $state<Status>("disconnected");
  /** Last connect error, surfaced under the power button. */
  error = $state<string | null>(null);

  private reapplyTimer: ReturnType<typeof setTimeout> | null = null;
  /** Signature of the config last applied, to avoid redundant reconnects. */
  private lastSig: string | null = null;

  /** Signature of everything that affects the generated config. */
  private configSig(): string {
    return JSON.stringify({
      server: subs.selectedServerId,
      mode: settings.vpnMode,
      killswitch: settings.killswitch,
      allowLan: settings.allowLan,
      split: splitInput(),
    });
  }

  /** Called reactively when config (location / split / mode / settings)
   *  changes: while connected, debounce-reconnect with the new config so the
   *  change takes effect live. The killswitch (if on) holds across the gap. */
  onConfigChanged(): void {
    const sig = this.configSig();
    if (this.lastSig === null) {
      this.lastSig = sig; // baseline on first run, no reconnect
      return;
    }
    if (sig === this.lastSig) return;
    this.lastSig = sig;
    if (this.status !== "connected" && this.status !== "connecting") return;
    if (this.reapplyTimer) clearTimeout(this.reapplyTimer);
    this.reapplyTimer = setTimeout(() => void this.connect(), 500);
  }

  async toggle(): Promise<void> {
    if (this.status === "connected" || this.status === "connecting") {
      await this.disconnect();
    } else {
      await this.connect();
    }
  }

  async connect(): Promise<void> {
    this.error = null;
    const server = subs.selectedServerRaw();
    if (!server) {
      this.error = t("conn.selectLocation");
      return;
    }
    this.status = "connecting";
    this.lastSig = this.configSig();
    try {
      const resp = await vpnConnect(
        server,
        splitInput(),
        settings.vpnMode,
        settings.killswitch,
        settings.allowLan,
      );
      if (!resp.ok) throw new Error(resp.error || "connection failed");
      this.status = "connected";
    } catch (e) {
      this.error = msg(e);
      this.status = "disconnected";
    }
  }

  async disconnect(): Promise<void> {
    try {
      await vpnDisconnect();
    } catch {
      // best effort — drop to disconnected regardless
    }
    this.status = "disconnected";
  }

  /** Reconcile UI with the helper's actual state (e.g. window recreated while
   *  still connected, or core crashed). */
  async refresh(): Promise<void> {
    try {
      const resp = await vpnStatus();
      if (resp.state === "connected") {
        this.status = "connected";
      } else if (resp.state === "disconnected" && this.status === "connected") {
        this.status = "disconnected";
      }
    } catch {
      // helper unreachable — leave UI as is
    }
  }
}

export const conn = new ConnStore();

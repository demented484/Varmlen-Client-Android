import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  coreActivate,
  coreInfo,
  coreInstall,
  coreUninstall,
  listCoreReleases,
  vpnDisconnect,
  type CoreInfo,
  type CoreProgress,
  type CoreRelease,
} from "$lib/api";
import { conn } from "$lib/conn.svelte";

function msg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** State + actions for the managed sing-box core: download / activate /
 *  uninstall multiple versions in parallel, with live progress for each.
 *
 *  - `info` reflects what's on disk (which versions, which one is active) and
 *    what's available on GitHub (`latest`, `has_update`).
 *  - `releases` is the GitHub release list, loaded on demand.
 *  - `progress[tag]` is the live download state for an in-flight install. */
class CoreStore {
  info = $state<CoreInfo | null>(null);
  releases = $state<CoreRelease[]>([]);
  releasesLoading = $state(false);
  checking = $state(false);
  error = $state<string | null>(null);

  /** Per-tag download progress (downloaded / total bytes + speed). */
  progress = $state<Record<string, CoreProgress>>({});
  /** Tags currently being installed (download in flight). */
  busyTags = $state<Set<string>>(new Set());
  /** Tag currently being activated or uninstalled, if any. */
  switchingTag = $state<string | null>(null);

  private progressUnlisten: UnlistenFn | null = null;

  /** Subscribe to backend download-progress events; called once at startup. */
  async startProgressListener(): Promise<void> {
    if (this.progressUnlisten) return;
    this.progressUnlisten = await listen<CoreProgress>("core://progress", (e) => {
      // Re-assign the whole map so $state notifies dependent UI nodes.
      const payload = e.payload;
      this.progress = { ...this.progress, [stripV(payload.tag)]: payload };
    });
  }

  async check(): Promise<void> {
    this.checking = true;
    try {
      this.info = await coreInfo();
      this.error = null;
    } catch (e) {
      this.error = msg(e);
    } finally {
      this.checking = false;
    }
  }

  async loadReleases(): Promise<void> {
    if (this.releases.length > 0) return;
    this.releasesLoading = true;
    try {
      this.releases = await listCoreReleases();
    } catch (e) {
      this.error = msg(e);
    } finally {
      this.releasesLoading = false;
    }
  }

  private markBusy(tag: string, busy: boolean): void {
    const next = new Set(this.busyTags);
    if (busy) next.add(tag);
    else next.delete(tag);
    this.busyTags = next;
  }

  /** Download `version` (GitHub tag like "v1.13.0"; null = latest release).
   *  First-install case automatically activates the new version. */
  async install(version: string | null = null): Promise<void> {
    // Pre-resolve the tag we'll see in progress events so the UI can show
    // the bar from the very first byte. For "latest" we don't know it yet —
    // fall back to a placeholder key and remap once install settles.
    const key = stripV(version ?? "latest");
    this.markBusy(key, true);
    this.error = null;
    try {
      await coreInstall(version);
      await this.check();
    } catch (e) {
      this.error = msg(e);
    } finally {
      this.markBusy(key, false);
      // Drop progress entries for any tag that finished — the bar disappears.
      const { [key]: _drop, ...rest } = this.progress;
      this.progress = rest;
    }
  }

  /** Switch to an already-downloaded version. */
  async activate(tag: string): Promise<void> {
    const t = stripV(tag);
    this.switchingTag = t;
    this.error = null;
    try {
      await coreActivate(t);
      await this.check();
    } catch (e) {
      this.error = msg(e);
    } finally {
      this.switchingTag = null;
    }
  }

  /** Delete a cached version from disk. If the user is deleting the version
   *  they're currently using, we tear down the VPN first — leaving the tunnel
   *  alive while its core binary disappears would be a hung connection. */
  async uninstall(tag: string): Promise<void> {
    const t = stripV(tag);
    this.switchingTag = t;
    this.error = null;
    try {
      if (this.isActive(t) && conn.status !== "disconnected") {
        await conn.disconnect();
      }
      await coreUninstall(t);
      await this.check();
    } catch (e) {
      this.error = msg(e);
    } finally {
      this.switchingTag = null;
    }
  }

  /** On launch: check the version, auto-install when nothing is cached at all
   *  (a core is required to connect). Existing installs are left alone so the
   *  user keeps whatever version they explicitly picked. */
  async autoInit(): Promise<void> {
    await this.startProgressListener();
    await this.check();
    if (this.info && this.info.installed.length === 0 && this.info.latest) {
      await this.install();
    }
  }

  /** Is `tag` currently downloaded? */
  isInstalled(tag: string): boolean {
    const t = stripV(tag);
    return this.info?.installed.some((v) => v.tag === t) ?? false;
  }

  /** Is `tag` the active version? */
  isActive(tag: string): boolean {
    return this.info?.active === stripV(tag);
  }
}

function stripV(s: string): string {
  return s.startsWith("v") ? s.slice(1) : s;
}

export const core = new CoreStore();

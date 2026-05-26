import { coreInfo, coreInstall, listCoreReleases, type CoreInfo, type CoreRelease } from "$lib/api";

function msg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** State + actions for the managed sing-box core (download / auto-update). */
class CoreStore {
  info = $state<CoreInfo | null>(null);
  /** True while an install/update is downloading. */
  busy = $state(false);
  /** True while the version check is in flight. */
  checking = $state(false);
  error = $state<string | null>(null);
  /** Available releases (loaded on demand for the version picker). */
  releases = $state<CoreRelease[]>([]);
  releasesLoading = $state(false);

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

  /** Install the latest release (default) or a specific version tag. */
  async install(version: string | null = null): Promise<void> {
    this.busy = true;
    this.error = null;
    try {
      await coreInstall(version);
      await this.check();
    } catch (e) {
      this.error = msg(e);
    } finally {
      this.busy = false;
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

  /** On launch: check the version, and auto-install when the core is missing
   *  entirely (it's required to connect). Updates are surfaced, not forced. */
  async autoInit(): Promise<void> {
    await this.check();
    if (this.info && this.info.installed == null && this.info.latest) {
      await this.install();
    }
  }
}

export const core = new CoreStore();

<script lang="ts">
  import { theme } from "$lib/theme.svelte";
  import { settings, type VpnMode } from "$lib/settings.svelte";
  import { i18n, t, LANGUAGES, type Lang } from "$lib/i18n.svelte";
  import { core } from "$lib/core.svelte";
  import { helperInstalled, installHelper } from "$lib/api";
  import Dropdown from "$lib/components/Dropdown.svelte";

  const modeOptions = $derived([
    { value: "tun", label: t("mode.tun") },
    { value: "proxy", label: t("mode.proxy") },
  ]);
  const modeSub = $derived(settings.vpnMode === "proxy" ? t("mode.proxySub") : t("mode.tunSub"));

  // Refresh the core status when Settings opens (cheap GitHub check).
  $effect(() => {
    void core.check();
  });

  let helperOk = $state(false);
  let helperBusy = $state(false);
  let helperErr = $state<string | null>(null);

  async function refreshHelper() {
    try {
      helperOk = await helperInstalled();
    } catch {
      helperOk = false;
    }
  }
  $effect(() => {
    void refreshHelper();
  });

  async function setupHelper() {
    helperBusy = true;
    helperErr = null;
    try {
      await installHelper();
      await refreshHelper();
    } catch (e) {
      helperErr = e instanceof Error ? e.message : String(e);
    } finally {
      helperBusy = false;
    }
  }

  const helperStatus = $derived(
    helperOk ? t("helper.ready") : helperErr ?? t("helper.notInstalled"),
  );

  let showVersions = $state(false);

  async function openVersions() {
    showVersions = true;
    await core.loadReleases();
  }

  async function installSpecific(tag: string) {
    showVersions = false;
    await core.install(tag);
  }

  function formatReleaseDate(d: string | null): string {
    if (!d) return "";
    const dt = new Date(d);
    if (!Number.isFinite(dt.getTime())) return "";
    const pad = (n: number) => n.toString().padStart(2, "0");
    return `${pad(dt.getDate())}.${pad(dt.getMonth() + 1)}.${dt.getFullYear()}`;
  }

  const coreStatus = $derived.by(() => {
    if (core.checking && !core.info) return t("core.checking");
    const info = core.info;
    if (!info) return core.error ? t("core.checkFailed") : t("core.checking");
    if (info.installed) {
      const tail = info.has_update ? t("core.updateAvailable") : t("core.upToDate");
      return `${info.installed} · ${tail}`;
    }
    return info.latest
      ? `${t("core.notInstalled")} · ${t("core.latest", { v: info.latest })}`
      : t("core.notInstalled");
  });

</script>

<header class="topbar">
  <h1>{t("settings.title")}</h1>
</header>

<main class="scroll">
  <section>
    <h2>{t("settings.appearance")}</h2>
    <div class="card theme-card">
      <div class="theme-row">
        <button
          class="theme-tile"
          class:active={theme.current === "dark"}
          onclick={() => theme.set("dark")}
          aria-pressed={theme.current === "dark"}
        >
          <div class="swatch swatch-dark"></div>
          <span>{t("settings.dark")}</span>
        </button>
        <button
          class="theme-tile"
          class:active={theme.current === "light"}
          onclick={() => theme.set("light")}
          aria-pressed={theme.current === "light"}
        >
          <div class="swatch swatch-light"></div>
          <span>{t("settings.light")}</span>
        </button>
      </div>
    </div>
  </section>

  <section>
    <h2>{t("settings.vpnMode")}</h2>
    <div class="list">
      <div class="row">
        <div class="row-text">
          <div class="row-title">{t("settings.vpnMode")}</div>
          <div class="row-sub muted">{modeSub}</div>
        </div>
        <Dropdown
          value={settings.vpnMode}
          options={modeOptions}
          onChange={(v) => settings.setVpnMode(v as VpnMode)}
          ariaLabel={t("settings.vpnMode")}
        />
      </div>
    </div>
  </section>

  <section>
    <h2>{t("settings.general")}</h2>
    <div class="list">
      <div class="row">
        <div class="row-text">
          <div class="row-title">{t("settings.language")}</div>
        </div>
        <Dropdown
          value={i18n.lang}
          options={LANGUAGES}
          onChange={(v) => i18n.set(v as Lang)}
          ariaLabel={t("settings.language")}
        />
      </div>
      <label class="row">
        <div class="row-text">
          <div class="row-title">{t("settings.killswitch")}</div>
          <div class="row-sub muted">{t("settings.killswitchSub")}</div>
        </div>
        <span class="switch">
          <input
            type="checkbox"
            checked={settings.killswitch}
            onchange={(e) => settings.setKillswitch((e.currentTarget as HTMLInputElement).checked)}
          />
          <span class="slider"></span>
        </span>
      </label>
      <label class="row">
        <div class="row-text">
          <div class="row-title">{t("settings.allowLan")}</div>
          <div class="row-sub muted">{t("settings.allowLanSub")}</div>
        </div>
        <span class="switch">
          <input
            type="checkbox"
            checked={settings.allowLan}
            onchange={(e) => settings.setAllowLan((e.currentTarget as HTMLInputElement).checked)}
          />
          <span class="slider"></span>
        </span>
      </label>
    </div>
  </section>

  <section>
    <h2>{t("settings.core")}</h2>
    <div class="list">
      <div class="row">
        <div class="row-text">
          <div class="row-title">sing-box</div>
          <div class="row-sub muted">{coreStatus}</div>
        </div>
        <button
          class="btn btn-ghost"
          onclick={openVersions}
          disabled={core.busy}
          title={t("core.versionsTitle")}
        >
          {t("core.versions")}
        </button>
        {#if !core.info || core.info.has_update}
          <button class="btn btn-primary" onclick={() => core.install()} disabled={core.busy}>
            {core.busy
              ? t("core.updating")
              : core.info?.installed
                ? t("core.update")
                : t("core.install")}
          </button>
        {/if}
      </div>
    </div>
  </section>

  {#if showVersions}
    <div class="modal-backdrop" onclick={() => (showVersions = false)} role="presentation">
      <div
        class="modal card"
        onclick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label={t("core.versionsTitle")}
      >
        <h2>{t("core.versionsTitle")}</h2>
        {#if core.releasesLoading && core.releases.length === 0}
          <p class="muted">{t("core.checking")}</p>
        {:else if core.error && core.releases.length === 0}
          <p style="color: var(--danger)">{core.error}</p>
        {:else}
          <ul class="ver-list">
            {#each core.releases as r (r.tag)}
              {@const ver = r.tag.replace(/^v/, "")}
              {@const isInstalled = core.info?.installed === ver}
              <li>
                <button
                  type="button"
                  class="ver-row"
                  class:current={isInstalled}
                  onclick={() => installSpecific(r.tag)}
                  disabled={core.busy || isInstalled}
                >
                  <span class="ver-tag">{r.tag}</span>
                  <span class="ver-meta muted">
                    {formatReleaseDate(r.date)}
                    {#if r.prerelease}<span class="badge">{t("core.preview")}</span>{/if}
                    {#if isInstalled}<span class="badge badge-on">{t("core.currentlyInstalled")}</span>{/if}
                  </span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
        <div class="modal-actions">
          <button class="btn" onclick={() => (showVersions = false)}>{t("common.close")}</button>
        </div>
      </div>
    </div>
  {/if}

  <section>
    <h2>{t("settings.helper")}</h2>
    <div class="list">
      <div class="row">
        <span
          class="status-dot"
          class:on={helperOk && !helperBusy}
          class:off={!helperOk && !helperBusy}
          class:busy={helperBusy}
        ></span>
        <div class="row-text">
          <div class="row-title">{t("helper.title")}</div>
          <div class="row-sub muted">{helperStatus}</div>
        </div>
        <button
          class="btn {helperOk ? '' : 'btn-primary'}"
          onclick={setupHelper}
          disabled={helperBusy}
        >
          {helperBusy
            ? t("helper.installing")
            : helperOk
              ? t("helper.reinstall")
              : t("helper.install")}
        </button>
        {#if helperErr}
          <div class="row-sub" style="color: var(--danger)">{helperErr}</div>
        {/if}
      </div>
    </div>
  </section>

</main>

<style>
  .topbar {
    display: flex;
    align-items: center;
    padding: 14px 16px 6px;
    flex-shrink: 0;
  }
  .topbar h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 700;
  }

  .scroll {
    position: absolute;
    inset: 56px 0 0 0;
    overflow-y: auto;
    scrollbar-gutter: stable;
    padding: 0 14px 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  h2 {
    margin: 0;
    padding: 0 4px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .theme-card { padding: 12px; }
  .theme-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .theme-tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 12px;
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--text);
  }
  .theme-tile.active {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-faint);
  }
  .swatch {
    width: 100%;
    height: 56px;
    border-radius: 8px;
    border: 1px solid var(--border);
  }
  .swatch-dark { background: linear-gradient(135deg, #1a1a1a 50%, #2e2e2e 50%); }
  .swatch-light { background: linear-gradient(135deg, #ffffff 50%, #ebebeb 50%); }

  /* Same .list-row layout as the design system, but using <label> so the
     entire row activates the toggle without the cell extending past the
     visual edge. */
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    cursor: pointer;
  }
  .row + .row {
    border-top: 1px solid var(--border);
  }
  .row:hover {
    background: var(--bg-elev-2);
  }
  .list {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .row-text {
    flex: 1;
    min-width: 0;
  }
  .row-title { font-size: 14px; }
  .row-sub { font-size: 12px; margin-top: 2px; }

  .status-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
    margin-right: 2px;
  }
  .status-dot.on {
    background: #2eb872;
    box-shadow: 0 0 0 3px rgba(46, 184, 114, 0.18);
  }
  .status-dot.off {
    background: var(--danger);
    box-shadow: 0 0 0 3px var(--danger-faint);
  }
  .status-dot.busy {
    background: var(--warn);
    box-shadow: 0 0 0 3px rgba(245, 165, 36, 0.22);
    animation: status-pulse 1.2s ease-in-out infinite;
  }
  @keyframes status-pulse {
    50% { opacity: 0.45; }
  }

  /* ---------- version-picker modal ---------- */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    padding: 16px;
  }
  .modal {
    width: min(440px, 100%);
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 18px;
  }
  .modal h2 {
    margin: 0;
    font-size: 16px;
    color: var(--text);
    text-transform: none;
    letter-spacing: 0;
    padding: 0;
  }
  .modal p { margin: 0; font-size: 13px; }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  .ver-list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-elev-2);
  }
  .ver-list li + li { border-top: 1px solid var(--border); }
  .ver-row {
    width: 100%;
    background: transparent;
    border: 0;
    color: var(--text);
    text-align: left;
    padding: 10px 12px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    cursor: pointer;
  }
  .ver-row:hover:not(:disabled) { background: var(--bg-elev); }
  .ver-row:disabled { cursor: default; opacity: 0.7; }
  .ver-row.current { color: var(--text-muted); }
  .ver-tag { font-weight: 600; font-size: 13px; }
  .ver-meta { font-size: 12px; display: flex; align-items: center; gap: 6px; }
  .badge {
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 999px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .badge-on {
    background: rgba(46, 184, 114, 0.18);
    border-color: rgba(46, 184, 114, 0.35);
    color: #2eb872;
  }
</style>

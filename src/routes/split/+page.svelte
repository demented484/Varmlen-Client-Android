<script lang="ts">
  import { split, type Mode } from "$lib/split.svelte";
  import { listInstalledApps, appFromFile, pickFile, type InstalledApp } from "$lib/api";
  import { t } from "$lib/i18n.svelte";
  import Dropdown from "$lib/components/Dropdown.svelte";

  type Tab = "apps" | "websites";
  let tab = $state<Tab>("apps");

  const modeOptions = $derived([
    { value: "general", label: t("split.modeGeneral") },
    { value: "selective", label: t("split.modeSelective") },
  ]);

  let appQuery = $state("");
  let siteDraft = $state("");
  let showAddApp = $state(false);
  let pickerQuery = $state("");
  let installed = $state<InstalledApp[]>([]);
  let pickerLoading = $state(false);

  const addedIds = $derived(new Set(split.currentApps.map((a) => a.id)));
  const pickerResults = $derived.by(() => {
    const q = pickerQuery.trim().toLowerCase();
    if (!q) return installed;
    return installed.filter(
      (a) => a.name.toLowerCase().includes(q) || a.id.toLowerCase().includes(q),
    );
  });

  async function openAddApp() {
    showAddApp = true;
    pickerQuery = "";
    if (installed.length > 0) return;
    pickerLoading = true;
    try {
      installed = await listInstalledApps();
    } catch {
      installed = [];
    } finally {
      pickerLoading = false;
    }
  }

  function addInstalled(app: InstalledApp) {
    split.addApp({ id: app.id, name: app.name, icon: app.icon ?? "📦" });
  }

  async function pickFromFile() {
    const picked = await pickFile();
    if (!picked) return;
    const app = await appFromFile(picked);
    if (app) {
      split.addApp({ id: app.id, name: app.name, icon: app.icon ?? "📦" });
    }
    showAddApp = false;
  }

  const filteredApps = $derived(
    split.currentApps.filter((a) =>
      a.name.toLowerCase().includes(appQuery.trim().toLowerCase()),
    ),
  );

  const currentMode = $derived(tab === "apps" ? split.appsMode : split.sitesMode);

  const enabledCount = $derived(
    tab === "apps"
      ? split.currentApps.filter((a) => a.enabled).length
      : split.currentSites.filter((s) => s.enabled).length,
  );

  const modeDescription = $derived(
    t(`split.mode.${tab === "apps" ? "apps" : "sites"}${currentMode === "selective" ? "Selective" : "General"}`),
  );

  function setMode(m: Mode) {
    if (tab === "apps") split.setAppsMode(m);
    else split.setSitesMode(m);
  }
</script>

{#snippet appIcon(icon: string | null | undefined)}
  {#if icon && icon.startsWith("data:")}
    <img class="app-icon" src={icon} alt="" />
  {:else}
    <span class="app-icon">{icon || "📦"}</span>
  {/if}
{/snippet}

<header class="topbar">
  <h1>{t("split.title")}</h1>
</header>

<div class="page">

  <div class="segmented" role="tablist">
    <button class:active={tab === "apps"} onclick={() => (tab = "apps")} role="tab" aria-selected={tab === "apps"}>{t("split.apps")}</button>
    <button class:active={tab === "websites"} onclick={() => (tab = "websites")} role="tab" aria-selected={tab === "websites"}>{t("split.websites")}</button>
  </div>

  <div class="card mode">
    <div class="mode-top">
      <div class="mode-label">
        <div class="mode-title">{t("split.mode")}</div>
        <div class="muted small">{t("split.active", { n: enabledCount })}</div>
      </div>
      <Dropdown
        value={currentMode}
        options={modeOptions}
        onChange={(v) => setMode(v as Mode)}
        ariaLabel={t("split.mode")}
      />
    </div>
    <p class="mode-note dim">{modeDescription}</p>
  </div>

  {#if tab === "apps"}
    <div class="apps-controls">
      <input class="search" type="search" placeholder={t("split.searchApps")} bind:value={appQuery} />
      <button class="btn btn-primary add-app" onclick={openAddApp} aria-label={t("split.addApp")}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" /></svg>
      </button>
    </div>

    {#if split.currentApps.length === 0}
      <div class="empty-state">
        <div class="empty-title">{t("split.noAppsTitle")}</div>
        <div class="muted">{t("split.noAppsHint")}</div>
      </div>
    {:else}
      <div class="list">
        {#each filteredApps as a (a.id)}
          <div class="list-row">
            {@render appIcon(a.icon)}
            <div class="app-text">
              <div class="app-name">{a.name}</div>
              <div class="app-id dim">{a.id}</div>
            </div>
            <button class="btn-ghost trash" onclick={() => split.removeApp(a.id)} aria-label="Remove">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none"><path d="M6 6l12 12M6 18L18 6" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg>
            </button>
            <label class="switch">
              <input type="checkbox" checked={a.enabled} onchange={() => split.toggleApp(a.id)} />
              <span class="slider"></span>
            </label>
          </div>
        {/each}
        {#if filteredApps.length === 0}
          <div class="list-row empty muted">{t("split.noAppsMatch")}</div>
        {/if}
      </div>
    {/if}
  {:else}
    <form class="site-add" onsubmit={(e) => { e.preventDefault(); split.addSite(siteDraft); siteDraft = ""; }}>
      <input type="text" placeholder={t("split.sitePlaceholder")} bind:value={siteDraft} />
      <button class="btn btn-primary" type="submit" disabled={!siteDraft.trim()}>{t("import.add")}</button>
    </form>

    {#if split.currentSites.length === 0}
      <div class="empty-state">
        <div class="empty-title">{t("split.noSitesTitle")}</div>
        <div class="muted">{t("split.noSitesHint")}</div>
      </div>
    {:else}
      <div class="list">
        {#each split.currentSites as s (s.id)}
          <div class="list-row">
            <span class="pattern">{s.pattern}</span>
            <button class="btn-ghost trash" onclick={() => split.removeSite(s.id)} aria-label="Remove">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none"><path d="M6 6l12 12M6 18L18 6" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg>
            </button>
            <label class="switch">
              <input type="checkbox" checked={s.enabled} onchange={() => split.toggleSite(s.id)} />
              <span class="slider"></span>
            </label>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

{#if showAddApp}
  <div class="modal-backdrop" onclick={() => (showAddApp = false)} role="presentation">
    <div
      class="modal card"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.key === "Escape" && (showAddApp = false)}
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label={t("split.addApp")}
    >
      <h2>{t("split.addApp")}</h2>
      <input type="search" placeholder={t("split.searchInstalled")} bind:value={pickerQuery} />

      <div class="picker">
        {#if pickerLoading}
          <div class="picker-msg muted">{t("split.loadingApps")}</div>
        {:else if pickerResults.length === 0}
          <div class="picker-msg muted">
            {installed.length === 0 ? t("split.noInstalled") : t("split.noInstalledMatch")}
          </div>
        {:else}
          {#each pickerResults as app (app.id)}
            <button
              class="picker-row"
              onclick={() => addInstalled(app)}
              disabled={addedIds.has(app.id)}
            >
              {@render appIcon(app.icon)}
              <div class="app-text">
                <div class="app-name">{app.name}</div>
                <div class="app-id dim">{app.id}</div>
              </div>
              {#if addedIds.has(app.id)}
                <svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true">
                  <path d="M5 12.5L10 17.5L19.5 8" stroke="var(--accent)" stroke-width="2.5" fill="none" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
              {/if}
            </button>
          {/each}
        {/if}
      </div>

      <p class="muted small-note">{t("split.pickFileHint")}</p>
      <div class="modal-actions">
        <button class="btn" onclick={() => (showAddApp = false)}>{t("common.close")}</button>
        <button class="btn btn-primary" onclick={pickFromFile}>{t("split.chooseFile")}</button>
      </div>
    </div>
  </div>
{/if}

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

  .page {
    position: absolute;
    inset: 56px 0 0 0;
    overflow-y: auto;
    /* Symmetric gutter — see +page.svelte for the rationale. */
    scrollbar-gutter: stable both-edges;
    padding: 0 14px 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  /* Tabs span the full width like every other panel. */
  .segmented {
    align-self: stretch;
    display: flex;
  }
  .segmented :global(button) {
    flex: 1;
  }
  .small {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .mode {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .mode-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .mode-label {
    min-width: 0;
  }
  .mode-title {
    font-size: 14px;
    font-weight: 600;
  }
  .mode-note {
    margin: 2px 0 0;
    font-size: 12px;
    padding: 8px 10px;
    background: var(--bg-elev-2);
    border-radius: var(--radius-sm);
  }

  .apps-controls {
    display: flex;
    gap: 8px;
  }
  .apps-controls .search { flex: 1; }
  .add-app {
    width: 38px;
    flex-shrink: 0;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .empty-state {
    padding: 28px 18px;
    text-align: center;
    background: var(--bg-elev);
    border: 1px dashed var(--border);
    border-radius: var(--radius);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .empty-title {
    font-weight: 600;
  }

  .app-icon {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    background: var(--bg-elev-2);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 16px;
    flex-shrink: 0;
  }
  img.app-icon {
    object-fit: contain;
    padding: 3px;
  }
  .app-text {
    flex: 1;
    min-width: 0;
  }
  .app-name {
    font-weight: 500;
  }
  .app-id {
    font-size: 11px;
    font-family: ui-monospace, "JetBrains Mono", monospace;
    margin-top: 1px;
  }

  .empty {
    justify-content: center;
    font-size: 13px;
  }

  .site-add {
    display: flex;
    gap: 8px;
  }
  .site-add input { flex: 1; }

  .pattern {
    flex: 1;
    font-family: ui-monospace, "JetBrains Mono", monospace;
    font-size: 13px;
  }
  .trash {
    width: 28px;
    height: 28px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .trash:hover { color: var(--danger); }


  /* modal */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: var(--overlay);
    display: flex;
    align-items: flex-end;
    justify-content: center;
    z-index: 100;
    animation: fadeIn var(--transition);
  }
  .modal {
    width: calc(100% - 24px);
    margin: 12px;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    animation: slideUp 180ms cubic-bezier(0.2, 0, 0, 1);
  }
  .modal h2 { margin: 0; font-size: 17px; font-weight: 600; }
  .modal p { margin: 0; font-size: 13px; }

  .picker {
    max-height: 320px;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-elev-2);
  }
  .picker-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    background: transparent;
    border: none;
    border-radius: 0;
    text-align: left;
    color: var(--text);
  }
  .picker-row + .picker-row {
    border-top: 1px solid var(--border);
  }
  .picker-row:hover:not(:disabled) {
    background: var(--bg-elev-3);
  }
  .picker-row:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .picker-msg {
    padding: 18px 12px;
    text-align: center;
    font-size: 13px;
  }
  .small-note {
    font-size: 12px;
  }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  @keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
  @keyframes slideUp {
    from { transform: translateY(20px); opacity: 0; }
    to   { transform: translateY(0); opacity: 1; }
  }
</style>

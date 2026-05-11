<script lang="ts">
  import { onMount } from "svelte";
  import { split, type Mode } from "$lib/split.svelte";

  type Tab = "apps" | "websites";
  let tab = $state<Tab>("apps");

  let appQuery = $state("");
  let siteDraft = $state("");
  let appDraftName = $state("");
  let appDraftId = $state("");
  let showAddApp = $state(false);

  onMount(() => split.init());

  const filteredApps = $derived(
    split.apps.filter((a) =>
      a.name.toLowerCase().includes(appQuery.trim().toLowerCase()),
    ),
  );

  const enabledCount = $derived(
    tab === "apps"
      ? split.apps.filter((a) => a.enabled).length
      : split.sites.filter((s) => s.enabled).length,
  );

  const modeDescription = $derived(
    split.mode === "selective"
      ? tab === "apps"
        ? "VPN works only for the selected apps. Everything else stays direct."
        : "VPN works only on the selected websites. Everything else stays direct."
      : tab === "apps"
        ? "VPN works for all apps except those selected (which stay direct)."
        : "VPN works on all websites except those selected (which stay direct).",
  );

  function setMode(m: Mode) {
    split.setMode(m);
  }

  function submitAddApp() {
    const id = appDraftId.trim();
    const name = appDraftName.trim() || id;
    if (!id) return;
    split.addApp({ id, name, icon: "📦" });
    appDraftId = "";
    appDraftName = "";
    showAddApp = false;
  }
</script>

<header class="topbar">
  <h1>Split tunneling</h1>
</header>

<div class="page">

  <div class="segmented" role="tablist">
    <button class:active={tab === "apps"} onclick={() => (tab = "apps")} role="tab" aria-selected={tab === "apps"}>Apps</button>
    <button class:active={tab === "websites"} onclick={() => (tab = "websites")} role="tab" aria-selected={tab === "websites"}>Websites</button>
  </div>

  <div class="card mode">
    <div class="mode-header">
      <span class="muted small">Mode</span>
      <span class="muted small">{enabledCount} active</span>
    </div>
    <label class="mode-row">
      <input type="radio" name="mode" checked={split.mode === "selective"} onchange={() => setMode("selective")} />
      <div>
        <div class="mode-title">Selective tunneling</div>
        <div class="mode-sub muted">VPN protects only the entries you pick.</div>
      </div>
    </label>
    <label class="mode-row">
      <input type="radio" name="mode" checked={split.mode === "general"} onchange={() => setMode("general")} />
      <div>
        <div class="mode-title">General tunneling</div>
        <div class="mode-sub muted">VPN protects everything except the entries you pick.</div>
      </div>
    </label>
    <p class="mode-note dim">{modeDescription}</p>
  </div>

  {#if tab === "apps"}
    <div class="apps-controls">
      <input class="search" type="search" placeholder="Search apps" bind:value={appQuery} />
      <button class="btn btn-primary add-app" onclick={() => (showAddApp = true)} aria-label="Add app">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" /></svg>
      </button>
    </div>

    {#if split.apps.length === 0}
      <div class="empty-state">
        <div class="empty-title">No apps yet</div>
        <div class="muted">Tap <strong>+</strong> to add a process by its name (e.g. <code>telegram-desktop</code>, <code>firefox</code>).</div>
      </div>
    {:else}
      <div class="list">
        {#each filteredApps as a (a.id)}
          <div class="list-row">
            <span class="app-icon">{a.icon}</span>
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
          <div class="list-row empty muted">No apps match the query.</div>
        {/if}
      </div>
      <p class="hint dim">
        Once the agent ships, AegisVPN will read installed apps from
        <code>/usr/share/applications/*.desktop</code> automatically.
      </p>
    {/if}
  {:else}
    <form class="site-add" onsubmit={(e) => { e.preventDefault(); split.addSite(siteDraft); siteDraft = ""; }}>
      <input type="text" placeholder="example.com or *.example.com" bind:value={siteDraft} />
      <button class="btn btn-primary" type="submit" disabled={!siteDraft.trim()}>Add</button>
    </form>

    {#if split.sites.length === 0}
      <div class="empty-state">
        <div class="empty-title">No websites yet</div>
        <div class="muted">Add a hostname (<code>example.com</code>) or a wildcard pattern (<code>*.example.com</code>).</div>
      </div>
    {:else}
      <div class="list">
        {#each split.sites as s (s.id)}
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
      aria-label="Add app"
    >
      <h2>Add app</h2>
      <p class="muted">Process / package name. The display name is optional.</p>
      <input type="text" placeholder="Process name (e.g. telegram-desktop)" bind:value={appDraftId} />
      <input type="text" placeholder="Display name (optional)" bind:value={appDraftName} />
      <div class="modal-actions">
        <button class="btn btn-ghost" onclick={() => (showAddApp = false)}>Cancel</button>
        <button class="btn btn-primary" onclick={submitAddApp} disabled={!appDraftId.trim()}>Add</button>
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
    padding: 0 14px 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .segmented {
    align-self: flex-start;
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
  .mode-header {
    display: flex;
    justify-content: space-between;
    margin-bottom: 4px;
  }
  .mode-row {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    cursor: pointer;
    padding: 4px 0;
  }
  .mode-row input {
    margin-top: 3px;
    width: auto;
    accent-color: var(--accent);
  }
  .mode-title {
    font-size: 14px;
    font-weight: 500;
  }
  .mode-sub {
    font-size: 12px;
    margin-top: 2px;
  }
  .mode-note {
    margin: 6px 0 0;
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

  .hint {
    font-size: 12px;
    padding: 0 4px;
  }
  code {
    font-family: ui-monospace, "JetBrains Mono", monospace;
    background: var(--bg-elev);
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 0.9em;
  }

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

<script lang="ts">
  type Mode = "selective" | "general";
  type Tab = "apps" | "websites";

  interface AppEntry {
    id: string;
    name: string;
    icon: string; // emoji placeholder until we wire .desktop scanning
    enabled: boolean;
  }
  interface SiteEntry {
    id: string;
    pattern: string;
    enabled: boolean;
  }

  let tab = $state<Tab>("apps");
  let mode = $state<Mode>("selective");

  let appQuery = $state("");
  let apps = $state<AppEntry[]>([
    { id: "telegram", name: "Telegram",   icon: "💬", enabled: true },
    { id: "discord",  name: "Discord",    icon: "🎮", enabled: true },
    { id: "firefox",  name: "Firefox",    icon: "🦊", enabled: false },
    { id: "chrome",   name: "Chromium",   icon: "🌐", enabled: false },
    { id: "spotify",  name: "Spotify",    icon: "🎵", enabled: false },
    { id: "steam",    name: "Steam",      icon: "🎯", enabled: true },
    { id: "obs",      name: "OBS Studio", icon: "📹", enabled: false },
    { id: "vscode",   name: "VS Code",    icon: "📝", enabled: false },
    { id: "sber",     name: "СберБанк",   icon: "🏦", enabled: false },
  ]);

  let siteDraft = $state("");
  let sites = $state<SiteEntry[]>([
    { id: "1", pattern: "*.ru",            enabled: true },
    { id: "2", pattern: "instagram.com",   enabled: true },
    { id: "3", pattern: "sberbank.ru",     enabled: true },
    { id: "4", pattern: "*.gov.ru",        enabled: true },
  ]);

  const filteredApps = $derived(
    apps.filter((a) =>
      a.name.toLowerCase().includes(appQuery.trim().toLowerCase()),
    ),
  );

  const enabledCount = $derived(
    tab === "apps"
      ? apps.filter((a) => a.enabled).length
      : sites.filter((s) => s.enabled).length,
  );

  function toggleApp(id: string) {
    apps = apps.map((a) => (a.id === id ? { ...a, enabled: !a.enabled } : a));
  }
  function toggleSite(id: string) {
    sites = sites.map((s) => (s.id === id ? { ...s, enabled: !s.enabled } : s));
  }
  function addSite() {
    const v = siteDraft.trim();
    if (!v) return;
    sites = [...sites, { id: crypto.randomUUID(), pattern: v, enabled: true }];
    siteDraft = "";
  }
  function removeSite(id: string) {
    sites = sites.filter((s) => s.id !== id);
  }

  const modeDescription = $derived(
    mode === "selective"
      ? tab === "apps"
        ? "VPN works only for the selected apps. Everything else stays direct."
        : "VPN works only on the selected websites. Everything else stays direct."
      : tab === "apps"
        ? "VPN works for all apps except those selected (which stay direct)."
        : "VPN works on all websites except those selected (which stay direct).",
  );
</script>

<div class="page">
  <header class="page-head">
    <h1>Split tunneling</h1>
  </header>

  <div class="segmented">
    <button class:active={tab === "apps"} onclick={() => (tab = "apps")}>Apps</button>
    <button class:active={tab === "websites"} onclick={() => (tab = "websites")}>Websites</button>
  </div>

  <div class="card mode">
    <div class="mode-header">
      <span class="muted small">Mode</span>
      <span class="muted small">{enabledCount} active</span>
    </div>
    <label class="mode-row">
      <input type="radio" bind:group={mode} value="selective" />
      <div>
        <div class="mode-title">Selective tunneling</div>
        <div class="mode-sub muted">VPN protects only the entries you pick.</div>
      </div>
    </label>
    <label class="mode-row">
      <input type="radio" bind:group={mode} value="general" />
      <div>
        <div class="mode-title">General tunneling</div>
        <div class="mode-sub muted">VPN protects everything except the entries you pick.</div>
      </div>
    </label>
    <p class="mode-note dim">{modeDescription}</p>
  </div>

  {#if tab === "apps"}
    <input class="search" type="search" placeholder="Search installed apps" bind:value={appQuery} />
    <div class="list">
      {#each filteredApps as a (a.id)}
        <div class="list-row">
          <span class="app-icon">{a.icon}</span>
          <div class="app-name">{a.name}</div>
          <label class="switch">
            <input type="checkbox" checked={a.enabled} onchange={() => toggleApp(a.id)} />
            <span class="slider"></span>
          </label>
        </div>
      {/each}
      {#if filteredApps.length === 0}
        <div class="list-row empty muted">No apps match the query.</div>
      {/if}
    </div>
    <p class="hint dim">App list is a placeholder. Once the agent ships, AegisVPN will read installed apps from <code>/usr/share/applications/*.desktop</code>.</p>
  {:else}
    <form class="site-add" onsubmit={(e) => { e.preventDefault(); addSite(); }}>
      <input type="text" placeholder="example.com or *.example.com" bind:value={siteDraft} />
      <button class="btn btn-primary" type="submit" disabled={!siteDraft.trim()}>Add</button>
    </form>

    <div class="list">
      {#each sites as s (s.id)}
        <div class="list-row">
          <span class="pattern">{s.pattern}</span>
          <button class="btn-ghost trash" onclick={() => removeSite(s.id)} aria-label="Remove">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
              <path d="M6 6l12 12M6 18L18 6" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
            </svg>
          </button>
          <label class="switch">
            <input type="checkbox" checked={s.enabled} onchange={() => toggleSite(s.id)} />
            <span class="slider"></span>
          </label>
        </div>
      {/each}
      {#if sites.length === 0}
        <div class="list-row empty muted">No sites yet.</div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .page-head h1 {
    margin: 0;
    font-size: 24px;
    font-weight: 700;
    padding: 4px 4px 4px;
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

  .search {
    margin-bottom: 0;
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
  .app-name {
    flex: 1;
    font-weight: 500;
  }

  .empty {
    justify-content: center;
    font-size: 13px;
  }

  .site-add {
    display: flex;
    gap: 8px;
  }
  .site-add input {
    flex: 1;
  }

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
  .trash:hover {
    color: var(--danger);
  }

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
</style>

<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { NAV } from "$lib/nav";
  import { t } from "$lib/i18n.svelte";
  import { core } from "$lib/core.svelte";
  import { conn } from "$lib/conn.svelte";
  import { subs } from "$lib/subs.svelte";
  import { split } from "$lib/split.svelte";
  import { settings } from "$lib/settings.svelte";
  import { readLegacyStorage, helperInstalled, installHelper } from "$lib/api";
  import "$lib/theme.svelte"; // module-level init applies persisted theme

  /** Trigger the privileged helper install (pkexec prompt) on the very first
   *  launch, once the core is ready. Tracked via localStorage so we don't
   *  prompt every time — the user can always do it manually in Settings. */
  async function maybeAutoInstallHelper() {
    if (typeof window === "undefined") return;
    if (localStorage.getItem("aegisvpn.helperAutoTried") === "1") return;
    try {
      if (await helperInstalled()) {
        localStorage.setItem("aegisvpn.helperAutoTried", "1");
        return;
      }
      localStorage.setItem("aegisvpn.helperAutoTried", "1");
      await installHelper();
    } catch (e) {
      console.warn("[helper] auto-install:", e);
    }
  }

  /** One-shot migration on first launch in a new origin (e.g. release vs dev
   *  use different WebKit storage). Pulls everything from the previous
   *  origin's localStorage and reloads so the stores re-init from it. */
  async function migrateLegacyStorage() {
    if (typeof window === "undefined") return;
    if (localStorage.getItem("aegisvpn.subs") !== null) return; // already seeded
    try {
      const data = await readLegacyStorage();
      const entries = Object.entries(data ?? {});
      console.log(`[migrate] legacy storage: ${entries.length} keys`);
      if (entries.length === 0) return;
      for (const [k, v] of entries) localStorage.setItem(k, v);
      window.location.reload();
    } catch (e) {
      console.error("[migrate] failed:", e);
    }
  }

  let { children } = $props();

  // Reading `page.url.pathname` through a $derived ensures the active-tab
  // class re-evaluates reliably on every navigation (intermittent stale state
  // otherwise).
  const currentPath = $derived(page.url.pathname);
  function isActive(path: string): boolean {
    if (path === "/") return currentPath === "/";
    return currentPath.startsWith(path);
  }

  // First-launch chores: migrate prior-origin localStorage, install the core
  // (auto), then prompt for the privileged helper (once).
  onMount(async () => {
    await migrateLegacyStorage();
    await core.autoInit();
    await maybeAutoInstallHelper();
  });

  // Reflect the real VPN state on launch: if the helper still runs sing-box
  // (e.g. the window was just recreated), show "connected" instead of a stale
  // "disconnected".
  onMount(() => void conn.refresh());

  // Live-reconnect when the config changes (location / split / mode / settings)
  // while connected. Reading these here registers them as effect dependencies.
  $effect(() => {
    void subs.selectedServerId;
    void settings.vpnMode;
    void settings.killswitch;
    void settings.allowLan;
    void split.appsMode;
    void split.sitesMode;
    void split.apps;
    void split.sites;
    conn.onConfigChanged();
  });
</script>

<div class="app">
  <main class="content">
    {@render children?.()}
  </main>

  <nav class="tabbar">
    {#each NAV as item}
      <a href={item.path} class="tab" class:active={isActive(item.path)}>
        <svg width="22" height="22" viewBox="0 0 24 24" aria-hidden="true">
          <path d={item.icon} fill="currentColor" />
        </svg>
        <span>{t(item.labelKey)}</span>
      </a>
    {/each}
  </nav>
</div>

<style>
  .app {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }

  .content {
    flex: 1;
    min-height: 0;
    position: relative;
    overflow: hidden;
  }

  .tabbar {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    border-top: 1px solid var(--border);
    background: var(--bg-elev);
    padding: 6px 4px 8px;
    padding-bottom: max(8px, env(safe-area-inset-bottom));
    flex-shrink: 0;
  }
  .tab {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 6px 4px;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 500;
    transition: color var(--transition);
    border-radius: var(--radius-sm);
  }
  .tab:hover { color: var(--text); }
  .tab.active { color: var(--accent); }
</style>

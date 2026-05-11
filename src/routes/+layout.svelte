<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { NAV } from "$lib/nav";
  import { theme } from "$lib/theme.svelte";

  let { children } = $props();

  onMount(() => theme.init());

  function isActive(path: string): boolean {
    if (path === "/") return page.url.pathname === "/";
    return page.url.pathname.startsWith(path);
  }
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
        <span>{item.label}</span>
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

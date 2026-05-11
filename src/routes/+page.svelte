<script lang="ts">
  import { onMount } from "svelte";
  import { conn, fmtElapsed } from "$lib/conn.svelte";
  import { subs } from "$lib/subs.svelte";

  let showImport = $state(false);
  let subUrl = $state("");
  let importError = $state<string | null>(null);
  let openMenuFor = $state<string | null>(null);

  onMount(() => subs.init());

  const statusLabel = $derived(
    {
      disconnected: "Not connected",
      connecting: "Connecting…",
      connected: "Connected",
    }[conn.status],
  );

  const allCollapsed = $derived(
    subs.list.length > 0 && subs.list.every((s) => s.collapsed),
  );

  async function importSubscription(): Promise<void> {
    if (!subUrl.trim()) return;
    importError = null;
    try {
      await subs.importFromUrl(subUrl);
      subUrl = "";
      showImport = false;
    } catch (e) {
      importError = e instanceof Error ? e.message : String(e);
    }
  }

  function fmtImported(iso: string): string {
    const d = new Date(iso);
    const pad = (n: number) => n.toString().padStart(2, "0");
    return `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }
</script>

<header class="topbar">
  <h1 class="app-title">AegisVPN</h1>
  <button class="icon-btn" onclick={() => (showImport = true)} aria-label="Add subscription">
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none">
      <path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" />
    </svg>
  </button>
</header>

<main class="scroll">
  <section class="hero">
    <button
      class="power"
      data-status={conn.status}
      data-traffic={conn.outgoing ? "on" : "off"}
      onclick={() => conn.toggle()}
      aria-label={conn.status === "connected" ? "Disconnect" : "Connect"}
    >
      <span class="halo halo-1"></span>
      <span class="halo halo-2"></span>
      <svg viewBox="0 0 64 64" width="62" height="62" aria-hidden="true">
        <path
          d="M22 18a16 16 0 1 0 20 0"
          stroke="currentColor"
          stroke-width="3.5"
          stroke-linecap="round"
          fill="none"
        />
        <line x1="32" y1="11" x2="32" y2="30" stroke="currentColor" stroke-width="3.5" stroke-linecap="round" />
      </svg>
    </button>
    <div class="status-text" data-status={conn.status}>{statusLabel}</div>
    {#if conn.status === "connected"}
      <div class="timer">{fmtElapsed(conn.elapsedSec)}</div>
    {/if}
  </section>

  <div class="actions-row">
    <button class="text-link" disabled={conn.status !== "connected"}>
      Check current connection
    </button>
    <button
      class="text-link"
      onclick={() => (allCollapsed ? subs.expandAll() : subs.collapseAll())}
    >
      {allCollapsed ? "Show all" : "Hide all"}
    </button>
  </div>

  {#each subs.list as sub (sub.id)}
    <section class="sub-card">
      <header class="sub-head">
        <button
          class="chev-toggle"
          onclick={() => subs.toggleCollapse(sub.id)}
          aria-label={sub.collapsed ? "Expand" : "Collapse"}
        >
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            class="chev-icon"
            style="transform: rotate({sub.collapsed ? -90 : 0}deg)"
          >
            <path d="M6 9l6 6 6-6" stroke="currentColor" stroke-width="2.2" fill="none" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>

        <div class="sub-info">
          <div class="sub-title">{sub.name}</div>
          <div class="sub-meta muted">
            {fmtImported(sub.importedAt)} · auto-update {sub.updateIntervalHours}h
          </div>
        </div>

        <button class="head-btn" onclick={() => subs.refresh(sub.id)} aria-label="Refresh">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
            <path d="M3 12a9 9 0 0 1 15.5-6.36M21 12a9 9 0 0 1-15.5 6.36M16 5h5V0M8 19H3v5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
        <button class="head-btn" onclick={() => subs.pingAll(sub.id)} aria-label="Test latency">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
            <path d="M12 14V8M8 14v-3M16 14v-2" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
            <circle cx="12" cy="12" r="9" stroke="currentColor" stroke-width="1.8" />
          </svg>
        </button>
        <div class="menu-wrap">
          <button
            class="head-btn"
            aria-label="Subscription menu"
            onclick={() => (openMenuFor = openMenuFor === sub.id ? null : sub.id)}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
              <circle cx="5" cy="12" r="1.6" /><circle cx="12" cy="12" r="1.6" /><circle cx="19" cy="12" r="1.6" />
            </svg>
          </button>
          {#if openMenuFor === sub.id}
            <div class="menu" role="menu">
              <button
                role="menuitem"
                onclick={() => { subs.remove(sub.id); openMenuFor = null; }}
                class="menu-item danger"
              >Remove subscription</button>
            </div>
          {/if}
        </div>
      </header>

      <div class="sub-traffic">
        <button class="info-dot" aria-label="Subscription info">i</button>
        <div class="traffic-bar">
          <span class="traffic-text">{subs.trafficText(sub)}</span>
        </div>
        {#if sub.supportUrl}
          <a
            class="tg-btn"
            href={sub.supportUrl}
            target="_blank"
            rel="noopener"
            aria-label="Open support"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
              <path d="M9.04 15.78L8.7 19.2c.41 0 .59-.18.81-.39l1.95-1.86 4.04 2.96c.74.41 1.27.19 1.46-.69l2.65-12.4c.23-1.05-.38-1.46-1.1-1.2L3.5 9.84c-1.02.4-1.01.97-.17 1.23l4.04 1.26 9.4-5.92c.44-.27.85-.12.52.18z" />
            </svg>
          </a>
        {/if}
      </div>

      {#if subs.expiresText(sub)}
        <div class="expires muted small">Expires: {subs.expiresText(sub)}</div>
      {/if}

      {#if !sub.collapsed}
        <ul class="server-list">
          {#each sub.servers as srv (srv.id)}
            <li
              class="srv-row"
              class:active={subs.selectedServerId === srv.id}
            >
              <button class="srv-btn" onclick={() => subs.selectServer(srv.id)}>
                <span class="srv-stripe"></span>
                <span class="flag">{srv.flag}</span>
                <div class="srv-info">
                  <div class="srv-name">{srv.name}</div>
                  <div class="srv-tr dim">{srv.transport}</div>
                </div>
                <span class="srv-ping muted" class:pinging={srv.pinging}>
                  {srv.pinging
                    ? "…"
                    : srv.pingMs !== null
                      ? `${srv.pingMs} ms`
                      : "n/d"}
                </span>
                <svg width="16" height="16" viewBox="0 0 24 24" class="chev" aria-hidden="true">
                  <path d="M9 6l6 6-6 6" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/each}

  {#if subs.list.length === 0}
    <div class="empty muted">
      No subscriptions yet. Tap <strong>+</strong> in the top-right corner.
    </div>
  {/if}
</main>

{#if showImport}
  <div class="modal-backdrop" onclick={() => (showImport = false)} role="presentation">
    <div
      class="modal card"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.key === "Escape" && (showImport = false)}
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label="Add subscription"
    >
      <h2>Add subscription</h2>
      <p class="muted">Paste a subscription URL or a single <code>vless://</code> link.</p>
      <input
        type="url"
        placeholder="https://… or vless://…"
        bind:value={subUrl}
        disabled={subs.importing}
      />
      {#if importError}
        <div class="error">{importError}</div>
      {/if}
      <div class="modal-actions">
        <button class="btn btn-ghost" onclick={() => (showImport = false)}>Cancel</button>
        <button class="btn btn-primary" onclick={importSubscription} disabled={subs.importing || !subUrl.trim()}>
          {subs.importing ? "Importing…" : "Add"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px 6px;
    flex-shrink: 0;
  }
  .app-title {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
    letter-spacing: 0.01em;
  }
  .icon-btn {
    width: 38px;
    height: 38px;
    padding: 0;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text);
    background: transparent;
    border: none;
  }
  .icon-btn:hover {
    background: var(--bg-elev-2);
  }

  .scroll {
    position: absolute;
    inset: 56px 0 0 0;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0 14px 24px;
  }

  /* ---------- power hero ---------- */
  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    /* extra top padding so the halo ripple (-28px) doesn't clip
       against the scroll container's top edge */
    padding: 38px 0 14px;
    position: relative;
  }
  .power {
    width: 168px;
    height: 168px;
    border-radius: 50%;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    color: var(--text-muted);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    position: relative;
    transition: all var(--transition);
    z-index: 1;
  }
  .power:hover {
    border-color: var(--border-strong);
    color: var(--text);
  }
  .power[data-status="connecting"] {
    color: var(--warn);
  }
  .power[data-status="connected"] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-on);
  }

  .halo {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 1px solid transparent;
    pointer-events: none;
    opacity: 0;
    transition: opacity var(--transition);
  }
  /* halos are visible only while traffic is actually flowing */
  .power[data-status="connected"][data-traffic="on"] .halo-1 {
    inset: -14px;
    border-color: var(--accent);
    opacity: 0.35;
    animation: ripple 2s ease-out infinite;
  }
  .power[data-status="connected"][data-traffic="on"] .halo-2 {
    inset: -28px;
    border-color: var(--accent);
    opacity: 0.18;
    animation: ripple 2s ease-out infinite 1s;
  }
  .power[data-status="connecting"] .halo-1 {
    inset: -10px;
    border-color: var(--warn);
    opacity: 0.45;
    animation: ripple 1.2s ease-out infinite;
  }
  @keyframes ripple {
    0% { transform: scale(0.92); opacity: 0.5; }
    100% { transform: scale(1.08); opacity: 0; }
  }

  .status-text {
    font-size: 13px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-muted);
    margin-top: 6px;
  }
  .status-text[data-status="connected"] { color: var(--accent); }
  .status-text[data-status="connecting"] { color: var(--warn); }

  .timer {
    font-variant-numeric: tabular-nums;
    font-size: 20px;
    font-weight: 600;
    margin-top: -4px;
  }

  /* ---------- actions row ---------- */
  .actions-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 4px 4px 10px;
  }
  .text-link {
    background: transparent;
    border: none;
    padding: 4px 0;
    color: var(--text-muted);
    font-size: 13px;
  }
  .text-link:hover:not(:disabled) {
    color: var(--text);
  }
  .text-link:disabled {
    color: var(--text-dim);
    opacity: 0.55;
    cursor: not-allowed;
  }

  /* ---------- subscription card ---------- */
  .sub-card {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    margin-bottom: 10px;
    overflow: hidden;
  }
  .sub-head {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 10px 8px 8px 6px;
  }
  .chev-toggle {
    width: 28px;
    height: 28px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    background: transparent;
    border: none;
  }
  .chev-icon {
    transition: transform var(--transition);
  }
  .sub-info {
    flex: 1;
    min-width: 0;
  }
  .sub-title {
    font-size: 16px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub-meta {
    font-size: 11px;
    margin-top: 1px;
  }
  .head-btn {
    width: 30px;
    height: 30px;
    padding: 0;
    border-radius: 50%;
    color: var(--text-muted);
    background: transparent;
    border: none;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .head-btn:hover {
    background: var(--bg-elev-2);
    color: var(--text);
  }

  .menu-wrap {
    position: relative;
  }
  .menu {
    position: absolute;
    top: 34px;
    right: 0;
    min-width: 200px;
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow);
    padding: 4px;
    z-index: 50;
  }
  .menu-item {
    width: 100%;
    text-align: left;
    padding: 8px 10px;
    border-radius: 6px;
    background: transparent;
    border: none;
    color: var(--text);
    font-size: 13px;
  }
  .menu-item:hover {
    background: var(--bg-elev-3);
  }
  .menu-item.danger {
    color: var(--danger);
  }

  .sub-traffic {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px 12px 10px;
  }
  .info-dot {
    width: 22px;
    height: 22px;
    padding: 0;
    border-radius: 50%;
    border: 1.5px solid var(--accent);
    color: var(--accent);
    font-size: 12px;
    font-style: italic;
    font-weight: 700;
    background: transparent;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .traffic-bar {
    flex: 1;
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: 100px;
    padding: 6px 14px;
    text-align: center;
  }
  .traffic-text {
    font-variant-numeric: tabular-nums;
    font-size: 13px;
    font-weight: 500;
  }
  .tg-btn {
    width: 28px;
    height: 28px;
    padding: 0;
    color: var(--accent);
    background: transparent;
    border: none;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .tg-btn:hover {
    transform: scale(1.06);
  }

  .expires {
    padding: 0 14px 8px;
    font-size: 11px;
  }
  .small {
    font-size: 11px;
  }

  /* ---------- server list ---------- */
  .server-list {
    list-style: none;
    margin: 0;
    padding: 4px 0 4px;
  }
  .srv-row {
    position: relative;
  }
  .srv-btn {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px 10px 14px;
    background: transparent;
    border: none;
    color: inherit;
    text-align: left;
    border-radius: 0;
  }
  .srv-btn:hover {
    background: var(--bg-elev-2);
  }
  .srv-stripe {
    position: absolute;
    left: 0;
    top: 4px;
    bottom: 4px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: transparent;
    transition: background var(--transition);
  }
  .srv-row.active .srv-stripe {
    background: var(--accent);
  }
  .srv-row.active .srv-btn {
    background: var(--accent-faint);
  }
  .flag {
    font-size: 22px;
    line-height: 1;
    flex-shrink: 0;
  }
  .srv-info {
    flex: 1;
    min-width: 0;
  }
  .srv-name {
    font-weight: 600;
    font-size: 14px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .srv-tr {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin-top: 2px;
  }
  .srv-ping {
    font-variant-numeric: tabular-nums;
    font-size: 12px;
  }
  .srv-ping.pinging {
    color: var(--text-dim);
    animation: blink 1s ease-in-out infinite;
  }
  @keyframes blink {
    0%, 100% { opacity: 0.45; }
    50% { opacity: 1; }
  }
  .chev {
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .empty {
    text-align: center;
    padding: 40px 16px;
    font-size: 13px;
  }

  /* ---------- modal ---------- */
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
    gap: 12px;
    animation: slideUp 180ms cubic-bezier(0.2, 0, 0, 1);
  }
  .modal h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
  }
  .modal p { margin: 0; font-size: 13px; }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  code {
    font-family: ui-monospace, "JetBrains Mono", monospace;
    background: var(--bg-elev-2);
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 0.9em;
  }
  .error {
    color: var(--danger);
    background: var(--danger-faint);
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    font-size: 12px;
  }
  @keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
  @keyframes slideUp {
    from { transform: translateY(20px); opacity: 0; }
    to   { transform: translateY(0); opacity: 1; }
  }
</style>

<script lang="ts">
  interface Server {
    id: string;
    flag: string;
    name: string;
    transport: string;
    pingMs: number | null;
  }

  let showImport = $state(false);
  let subUrl = $state("");
  let importing = $state(false);
  let selectedId = $state("fi");

  let servers = $state<Server[]>([
    { id: "fi", flag: "🇫🇮", name: "Finland Exp", transport: "VLESS / XHTTP / REALITY", pingMs: 439 },
    { id: "se", flag: "🇸🇪", name: "Sweden | Stockholm", transport: "VLESS / XHTTP / REALITY", pingMs: null },
    { id: "us", flag: "🇺🇸", name: "USA | New York", transport: "VLESS / XHTTP / REALITY", pingMs: null },
  ]);

  function select(id: string) {
    selectedId = id;
    // TODO: invoke('set_active_server', { id })
  }

  async function importSubscription() {
    if (!subUrl.trim()) return;
    importing = true;
    try {
      // TODO: invoke('import_subscription', { url: subUrl })
      await new Promise((r) => setTimeout(r, 600));
      subUrl = "";
      showImport = false;
    } finally {
      importing = false;
    }
  }
</script>

<div class="page">
  <header class="page-head">
    <h1>Servers</h1>
    <button class="add-btn" onclick={() => (showImport = true)} aria-label="Add subscription">
      <svg width="20" height="20" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="2" stroke-linecap="round" fill="none" /></svg>
    </button>
  </header>

  <div class="list">
    {#each servers as s}
      <button class="list-row tappable srv" onclick={() => select(s.id)}>
        <span class="flag">{s.flag}</span>
        <div class="srv-info">
          <div class="srv-name">{s.name}</div>
          <div class="srv-transport dim">{s.transport}</div>
        </div>
        <span class="srv-ping muted">{s.pingMs !== null ? `${s.pingMs} ms` : "n/d"}</span>
        {#if selectedId === s.id}
          <svg class="check" width="20" height="20" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M5 12.5L10 17.5L19.5 8" stroke="var(--accent)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" fill="none" />
          </svg>
        {/if}
      </button>
    {/each}
  </div>

  {#if servers.length === 0}
    <div class="empty card">
      <p>No servers yet</p>
      <button class="btn btn-primary" onclick={() => (showImport = true)}>Add subscription</button>
    </div>
  {/if}
</div>

{#if showImport}
  <div
    class="modal-backdrop"
    onclick={() => (showImport = false)}
    role="presentation"
  >
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
        disabled={importing}
      />
      <div class="modal-actions">
        <button class="btn btn-ghost" onclick={() => (showImport = false)}>Cancel</button>
        <button
          class="btn btn-primary"
          onclick={importSubscription}
          disabled={importing || !subUrl.trim()}
        >
          {importing ? "Importing…" : "Add"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .page-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 4px 8px;
  }
  .page-head h1 {
    margin: 0;
    font-size: 24px;
    font-weight: 700;
  }
  .add-btn {
    width: 36px;
    height: 36px;
    padding: 0;
    border-radius: 50%;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    color: var(--text);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .add-btn:hover {
    background: var(--bg-elev-2);
  }

  .srv {
    width: 100%;
    text-align: left;
    color: inherit;
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
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .srv-transport {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin-top: 2px;
  }
  .srv-ping {
    font-variant-numeric: tabular-nums;
    font-size: 12px;
  }
  .check {
    flex-shrink: 0;
  }

  .empty {
    padding: 24px 18px;
    text-align: center;
    display: flex;
    flex-direction: column;
    gap: 10px;
    align-items: center;
  }
  .empty p {
    margin: 0;
    color: var(--text-muted);
  }

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
    width: 100%;
    max-width: 480px;
    margin: 16px;
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
  .modal p {
    margin: 0;
    font-size: 13px;
  }
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
  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  @keyframes slideUp {
    from { transform: translateY(20px); opacity: 0; }
    to { transform: translateY(0); opacity: 1; }
  }
</style>

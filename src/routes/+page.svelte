<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { conn } from "$lib/conn.svelte";
  import { subs } from "$lib/subs.svelte";
  import { t } from "$lib/i18n.svelte";

  import type { Subscription, ServerEntry } from "$lib/subs.svelte";

  let showImport = $state(false);
  let subUrl = $state("");
  let importError = $state<string | null>(null);
  let openMenuFor = $state<string | null>(null);
  let infoFor = $state<Subscription | null>(null);
  let renameFor = $state<Subscription | null>(null);
  let renameDraft = $state("");
  let detailFor = $state<ServerEntry | null>(null);
  // Fixed-position coords for the "…" menu so it escapes the card's overflow:hidden.
  let menuPos = $state({ top: 0, right: 0 });

  function toggleMenu(subId: string, e: MouseEvent) {
    if (openMenuFor === subId) {
      openMenuFor = null;
      return;
    }
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menuPos = { top: r.bottom + 4, right: window.innerWidth - r.right };
    openMenuFor = subId;
  }

  /** The parsed vless:// fields, as label/value rows for the detail modal. */
  const detailRows = $derived.by(() => {
    const s = detailFor?.raw;
    if (!s) return [] as Array<[string, string]>;
    const rows: Array<[string, string | null]> = [
      ["Address", `${s.host}:${s.port}`],
      ["UUID", s.uuid],
      ["Transport", s.transport],
      ["Security", s.security],
      ["SNI", s.sni],
      ["Fingerprint", s.fingerprint],
      ["Public key (pbk)", s.public_key],
      ["Short ID (sid)", s.short_id],
      ["Flow", s.flow],
      ["Path", s.path],
      ["Mode", s.mode],
      ["Packet encoding", s.packet_encoding],
    ];
    return rows.filter(([, v]) => v != null && v !== "") as Array<[string, string]>;
  });

  function openInfo(sub: Subscription) {
    infoFor = sub;
    openMenuFor = null;
  }
  function openRename(sub: Subscription) {
    renameFor = sub;
    renameDraft = sub.name;
    openMenuFor = null;
  }
  function commitRename() {
    if (renameFor) subs.rename(renameFor.id, renameDraft);
    renameFor = null;
  }

  // Subscription headers (support / web-page URLs) are attacker-controlled, so
  // only hand the OS opener a vetted web/Telegram scheme — never file:, etc.
  const SAFE_SCHEMES = new Set(["http:", "https:", "tg:"]);
  async function open(url: string | null) {
    if (!url) return;
    let scheme: string;
    try {
      scheme = new URL(url).protocol;
    } catch {
      return;
    }
    if (SAFE_SCHEMES.has(scheme)) await openUrl(url);
  }

  const statusLabel = $derived(t(`status.${conn.status}`));


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
      onclick={() => conn.toggle()}
      aria-label={conn.status === "connected" ? "Disconnect" : "Connect"}
    >
      <svg viewBox="0 0 64 64" width="54" height="54" class="power-icon" aria-hidden="true">
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
    {#if conn.error}
      <div class="conn-error">{conn.error}</div>
    {/if}
  </section>

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
          {#if sub.updateIntervalHours}
            <div class="sub-meta muted">{t("home.autoUpdate", { h: sub.updateIntervalHours })}</div>
          {/if}
        </div>

        <button
          class="head-btn"
          class:spinning={sub.refreshing}
          onclick={() => subs.refresh(sub.id)}
          aria-label="Refresh"
          disabled={sub.refreshing}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
            <path d="M21 12a9 9 0 1 1-3.13-6.84M21 4v5h-5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
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
            onclick={(e) => toggleMenu(sub.id, e)}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
              <circle cx="5" cy="12" r="1.6" /><circle cx="12" cy="12" r="1.6" /><circle cx="19" cy="12" r="1.6" />
            </svg>
          </button>
          {#if openMenuFor === sub.id}
            <div class="menu-backdrop" role="presentation" onclick={() => (openMenuFor = null)}></div>
            <div class="menu" role="menu" style="top: {menuPos.top}px; right: {menuPos.right}px;">
              <button role="menuitem" class="menu-item" onclick={() => openInfo(sub)}>
                {t("menu.info")}
              </button>
              <button role="menuitem" class="menu-item" onclick={() => openRename(sub)}>
                {t("menu.rename")}
              </button>
              <button
                role="menuitem"
                class="menu-item danger"
                onclick={() => { subs.remove(sub.id); openMenuFor = null; }}
              >
                {t("menu.remove")}
              </button>
            </div>
          {/if}
        </div>
      </header>

      <div class="sub-traffic">
        {#if sub.webPageUrl}
          <button class="round-btn" aria-label="Website" onclick={() => open(sub.webPageUrl)}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <circle cx="12" cy="12" r="9" />
              <path d="M12 16v-4" />
              <path d="M12 8h.01" />
            </svg>
          </button>
        {/if}
        <div class="traffic-bar">
          <span class="traffic-text">{subs.trafficText(sub)}</span>
        </div>
        {#if sub.supportUrl}
          <button class="round-btn" aria-label="Telegram" onclick={() => open(sub.supportUrl)}>
            <svg width="23" height="23" viewBox="0 0 128 128" fill="currentColor" aria-hidden="true">
              <path d="M28.9700376,63.3244248 C47.6273373,55.1957357 60.0684594,49.8368063 66.2934036,47.2476366 C84.0668845,39.855031 87.7600616,38.5708563 90.1672227,38.528 C90.6966555,38.5191258 91.8804274,38.6503351 92.6472251,39.2725385 C93.294694,39.7979149 93.4728387,40.5076237 93.5580865,41.0057381 C93.6433345,41.5038525 93.7494885,42.63857 93.6651041,43.5252052 C92.7019529,53.6451182 88.5344133,78.2034783 86.4142057,89.5379542 C85.5170662,94.3339958 83.750571,95.9420841 82.0403991,96.0994568 C78.3237996,96.4414641 75.5015827,93.6432685 71.9018743,91.2836143 C66.2690414,87.5912212 63.0868492,85.2926952 57.6192095,81.6896017 C51.3004058,77.5256038 55.3966232,75.2369981 58.9976911,71.4967761 C59.9401076,70.5179421 76.3155302,55.6232293 76.6324771,54.2720454 C76.6721165,54.1030573 76.7089039,53.4731496 76.3346867,53.1405352 C75.9604695,52.8079208 75.4081573,52.921662 75.0095933,53.0121213 C74.444641,53.1403447 65.4461175,59.0880351 48.0140228,70.8551922 C45.4598218,72.6091037 43.1463059,73.4636682 41.0734751,73.4188859 C38.7883453,73.3695169 34.3926725,72.1268388 31.1249416,71.0646282 C27.1169366,69.7617838 23.931454,69.0729605 24.208838,66.8603276 C24.3533167,65.7078514 25.9403832,64.5292172 28.9700376,63.3244248 Z" />
            </svg>
          </button>
        {/if}
      </div>

      {#if sub.description}
        <div class="description">{sub.description}</div>
      {/if}

      {#if subs.expiresText(sub)}
        <div class="expires muted small">{t("home.expires", { date: subs.expiresText(sub) ?? "" })}</div>
      {/if}

      {#if !sub.collapsed}
        <ul class="server-list">
          {#each sub.servers as srv (srv.id)}
            <li
              class="srv-row"
              class:active={subs.selectedServerId === srv.id}
            >
              <span class="srv-stripe"></span>
              <button class="srv-btn" onclick={() => subs.selectServer(srv.id)}>
                <span class="flag">{srv.flag ?? ""}</span>
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
              </button>
              <button
                class="srv-detail"
                aria-label="Location details"
                onclick={() => (detailFor = srv)}
              >
                <span class="chev-hit">
                  <svg width="16" height="16" viewBox="0 0 24 24" class="chev" aria-hidden="true">
                    <path d="M9 6l6 6-6 6" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round" />
                  </svg>
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/each}

  {#if subs.list.length === 0}
    <div class="empty muted">{t("home.empty")}</div>
  {/if}
</main>

{#if infoFor}
  <div class="modal-backdrop" onclick={() => (infoFor = null)} role="presentation">
    <div
      class="modal card"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.key === "Escape" && (infoFor = null)}
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label="Subscription info"
    >
      <h2>{infoFor.name}</h2>
      <dl class="info-grid">
        <dt>{t("info.url")}</dt>
        <dd class="mono small">{infoFor.url}</dd>

        <dt>{t("info.imported")}</dt>
        <dd>{fmtImported(infoFor.importedAt)}</dd>

        {#if infoFor.updateIntervalHours}
          <dt>{t("info.autoUpdate")}</dt>
          <dd>{t("info.everyH", { h: infoFor.updateIntervalHours })}</dd>
        {/if}

        <dt>{t("info.traffic")}</dt>
        <dd>{subs.trafficText(infoFor)}</dd>

        {#if subs.expiresText(infoFor)}
          <dt>{t("info.expires")}</dt>
          <dd>{subs.expiresText(infoFor)}</dd>
        {/if}

        <dt>{t("info.servers")}</dt>
        <dd>{infoFor.servers.length}</dd>

        {#if infoFor.supportUrl}
          <dt>{t("info.support")}</dt>
          <dd><a href={infoFor.supportUrl} target="_blank" rel="noopener">{infoFor.supportUrl}</a></dd>
        {/if}
      </dl>
      {#if infoFor.description}
        <p class="info-desc">{infoFor.description}</p>
      {/if}
      <div class="modal-actions">
        <button class="btn" onclick={() => (infoFor = null)}>{t("common.close")}</button>
      </div>
    </div>
  </div>
{/if}

{#if detailFor}
  <div class="modal-backdrop" onclick={() => (detailFor = null)} role="presentation">
    <div
      class="modal card"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.key === "Escape" && (detailFor = null)}
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label="Location details"
    >
      <h2>{detailFor.flag ? detailFor.flag + " " : ""}{detailFor.name}</h2>
      <dl class="info-grid">
        {#each detailRows as [label, value] (label)}
          <dt>{label}</dt>
          <dd class="mono small">{value}</dd>
        {/each}
      </dl>
      <div class="modal-actions">
        <button class="btn" onclick={() => (detailFor = null)}>{t("common.close")}</button>
      </div>
    </div>
  </div>
{/if}

{#if renameFor}
  <div class="modal-backdrop" onclick={() => (renameFor = null)} role="presentation">
    <div
      class="modal card"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.key === "Escape" && (renameFor = null)}
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label="Rename subscription"
    >
      <h2>{t("rename.title")}</h2>
      <input
        type="text"
        bind:value={renameDraft}
        onkeydown={(e) => e.key === "Enter" && commitRename()}
      />
      <div class="modal-actions">
        <button class="btn btn-ghost" onclick={() => (renameFor = null)}>{t("common.cancel")}</button>
        <button class="btn btn-primary" onclick={commitRename} disabled={!renameDraft.trim()}>
          {t("common.save")}
        </button>
      </div>
    </div>
  </div>
{/if}

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
      <h2>{t("import.title")}</h2>
      <p class="muted">{t("import.hint")}</p>
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
        <button class="btn btn-ghost" onclick={() => (showImport = false)}>{t("common.cancel")}</button>
        <button class="btn btn-primary" onclick={importSubscription} disabled={subs.importing || !subUrl.trim()}>
          {subs.importing ? t("import.importing") : t("import.add")}
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
    /* Reserve a gutter on BOTH sides so the visible gap from app-edge to
       panel-edge is identical on the left and on the right, regardless of
       whether the scrollbar is currently showing. With plain `stable`
       only the right side gets the gutter, and panels look pushed left. */
    scrollbar-gutter: stable both-edges;
    padding: 0 14px 24px;
  }

  /* ---------- power hero ---------- */
  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    /* Extra bottom padding pushes the subscriptions list down so it doesn't
       feel glued to the CONNECTED status text. */
    padding: 38px 0 32px;
    position: relative;
  }
  .power {
    width: 168px;
    height: 168px;
    border-radius: 50%;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    color: var(--text-muted);
    /* The icon is always centred; the timer is absolutely positioned below it
       so connecting/connected never shifts the icon up. */
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    position: relative;
    transition: background var(--transition), border-color var(--transition),
      color var(--transition);
    z-index: 1;
  }
  .power:hover {
    border-color: var(--border-strong);
    color: var(--text);
  }
  .power[data-status="connecting"] {
    color: var(--accent);
  }
  /* A spinning ring makes "connecting" unmistakable (motion is visible
     regardless of theme colour, and keeps the webview repainting). */
  .power[data-status="connecting"]::after {
    content: "";
    position: absolute;
    inset: -7px;
    border-radius: 50%;
    border: 3px solid var(--border);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }
  .power[data-status="connected"] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-on);
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
  .status-text[data-status="connecting"] { color: var(--accent); }
  .conn-error {
    margin-top: 6px;
    max-width: 280px;
    text-align: center;
    font-size: 12px;
    color: var(--danger);
    line-height: 1.4;
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
  .head-btn.spinning svg {
    animation: spin 900ms linear infinite;
    color: var(--accent);
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .menu-wrap {
    position: relative;
  }
  .menu {
    position: fixed;
    min-width: 200px;
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow);
    padding: 4px;
    z-index: 200;
  }
  .menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 199;
    background: transparent;
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
  .round-btn {
    width: 30px;
    height: 30px;
    padding: 0;
    border-radius: 50%;
    border: none;
    color: var(--accent);
    background: transparent;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .round-btn:hover {
    background: var(--accent-faint);
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
  .description {
    padding: 0 14px 8px;
    font-size: 12px;
    color: var(--text-muted);
    white-space: pre-line;
    line-height: 1.4;
  }
  .expires {
    padding: 0 14px 8px;
    font-size: 11px;
  }
  .info-grid {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 14px;
    margin: 0;
  }
  .info-grid dt {
    color: var(--text-muted);
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .info-grid dd {
    margin: 0;
    word-break: break-all;
    font-size: 13px;
  }
  .info-grid a {
    color: var(--accent);
    text-decoration: underline;
  }
  .mono {
    font-family: ui-monospace, "JetBrains Mono", monospace;
  }
  .info-desc {
    background: var(--bg-elev-2);
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    margin: 0;
    font-size: 12px;
    white-space: pre-line;
    line-height: 1.45;
    color: var(--text-muted);
  }
  .small {
    font-size: 11px;
  }

  /* ---------- server list ---------- */
  .server-list {
    list-style: none;
    margin: 0;
    /* no bottom padding: the last row reaches the card's rounded bottom edge,
       which clips it (sub-card has overflow:hidden) so its highlight fills the
       corner instead of leaving a dark gap */
    padding: 4px 0 0;
  }
  .srv-row {
    position: relative;
    display: flex;
    align-items: stretch;
    background: transparent;
    transition: background var(--transition);
  }
  /* Hovering anywhere on the row highlights the whole row. */
  .srv-row:hover {
    background: var(--bg-elev-2);
  }
  .srv-btn {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 4px 10px 14px;
    background: transparent;
    border: none;
    color: inherit;
    text-align: left;
    border-radius: 0;
  }
  .srv-detail {
    flex-shrink: 0;
    width: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    border-radius: 0;
    color: var(--text-dim);
  }
  .chev-hit {
    display: flex;
    align-items: center;
    justify-content: center;
  }
  /* Hovering the arrow itself just makes it brighter — no separate backdrop. */
  .srv-detail:hover {
    color: var(--text);
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
  .srv-row.active {
    background: var(--accent-faint);
  }
  .flag {
    font-size: 22px;
    line-height: 1;
    flex-shrink: 0;
    /* fixed slot so rows without a flag emoji keep the same text indent */
    width: 26px;
    text-align: center;
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
    color: inherit;
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

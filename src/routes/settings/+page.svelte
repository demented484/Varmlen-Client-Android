<script lang="ts">
  import { goto } from "$app/navigation";
  import { theme, type Theme } from "$lib/theme.svelte";

  let autostart = $state(false);
  let killswitch = $state(true);
  let allowLan = $state(true);
  let logLevel = $state<"warn" | "info" | "debug">("warn");

  function setTheme(t: Theme) {
    theme.set(t);
  }
</script>

<header class="topbar">
  <h1>Settings</h1>
</header>

<main class="scroll">
  <section>
    <h2>Appearance</h2>
    <div class="card theme-card">
      <div class="theme-row">
        <button
          class="theme-tile"
          class:active={theme.current === "dark"}
          onclick={() => setTheme("dark")}
          aria-pressed={theme.current === "dark"}
        >
          <div class="swatch swatch-dark"></div>
          <span>Dark</span>
        </button>
        <button
          class="theme-tile"
          class:active={theme.current === "light"}
          onclick={() => setTheme("light")}
          aria-pressed={theme.current === "light"}
        >
          <div class="swatch swatch-light"></div>
          <span>Light</span>
        </button>
      </div>
    </div>
  </section>

  <section>
    <h2>Routing</h2>
    <div class="list">
      <button class="list-row tappable" onclick={() => goto("/split")}>
        <div class="row-text">
          <div class="row-title">Split tunneling</div>
          <div class="row-sub muted">Per-app and per-website rules</div>
        </div>
        <svg width="16" height="16" viewBox="0 0 24 24" class="chev" aria-hidden="true">
          <path d="M9 6l6 6-6 6" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" />
        </svg>
      </button>
    </div>
  </section>

  <section>
    <h2>General</h2>
    <div class="list">
      <div class="list-row">
        <div class="row-text">
          <div class="row-title">Launch on system startup</div>
          <div class="row-sub muted">Open AegisVPN automatically after login.</div>
        </div>
        <label class="switch">
          <input type="checkbox" bind:checked={autostart} />
          <span class="slider"></span>
        </label>
      </div>
      <div class="list-row">
        <div class="row-text">
          <div class="row-title">Killswitch</div>
          <div class="row-sub muted">Block all traffic if the VPN connection drops.</div>
        </div>
        <label class="switch">
          <input type="checkbox" bind:checked={killswitch} />
          <span class="slider"></span>
        </label>
      </div>
      <div class="list-row">
        <div class="row-text">
          <div class="row-title">Allow LAN traffic</div>
          <div class="row-sub muted">Keep printers, NAS, and local devices reachable.</div>
        </div>
        <label class="switch">
          <input type="checkbox" bind:checked={allowLan} />
          <span class="slider"></span>
        </label>
      </div>
    </div>
  </section>

  <section>
    <h2>Diagnostics</h2>
    <div class="list">
      <div class="list-row">
        <div class="row-text">
          <div class="row-title">Log level</div>
          <div class="row-sub muted">Use <code>debug</code> only when reporting bugs.</div>
        </div>
        <select bind:value={logLevel} class="small-select">
          <option value="warn">Warn</option>
          <option value="info">Info</option>
          <option value="debug">Debug</option>
        </select>
      </div>
      <button class="list-row tappable">
        <div class="row-text">
          <div class="row-title">Open log directory</div>
          <div class="row-sub muted">Show the folder containing sing-box logs.</div>
        </div>
        <svg width="16" height="16" viewBox="0 0 24 24" class="chev" aria-hidden="true">
          <path d="M9 6l6 6-6 6" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" />
        </svg>
      </button>
    </div>
  </section>

  <section>
    <h2>About</h2>
    <div class="card about">
      <div>AegisVPN <span class="muted">v0.1.0</span></div>
      <div class="muted small">Open-source sing-box client. Licensed under AGPL-3.0.</div>
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
    padding: 0 14px 24px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 6px;
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

  .theme-card {
    padding: 12px;
  }
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

  .row-text {
    flex: 1;
    min-width: 0;
    text-align: left;
  }
  .row-title { font-size: 14px; }
  .row-sub { font-size: 12px; margin-top: 2px; }

  .small-select {
    width: auto;
    padding: 6px 10px;
    font-size: 13px;
  }
  .chev { color: var(--text-muted); }

  .about {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .small { font-size: 12px; }
  code {
    font-family: ui-monospace, "JetBrains Mono", monospace;
    background: var(--bg-elev-2);
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 0.9em;
  }
</style>

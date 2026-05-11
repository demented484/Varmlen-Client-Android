<script lang="ts">
  import { goto } from "$app/navigation";

  type Status = "disconnected" | "connecting" | "connected";

  let status = $state<Status>("disconnected");
  let currentServer = $state<{ flag: string; name: string; pingMs: number | null } | null>({
    flag: "🇫🇮",
    name: "Finland Exp",
    pingMs: 439,
  });

  let elapsedSec = $state(0);
  let timer: ReturnType<typeof setInterval> | null = null;

  const statusLabel = $derived(
    {
      disconnected: "Not connected",
      connecting: "Connecting…",
      connected: "Protected",
    }[status],
  );

  function pad(n: number): string {
    return n.toString().padStart(2, "0");
  }
  function fmt(s: number): string {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    return h ? `${h}:${pad(m)}:${pad(sec)}` : `${pad(m)}:${pad(sec)}`;
  }

  async function toggle() {
    if (status === "disconnected") {
      if (!currentServer) {
        goto("/servers");
        return;
      }
      status = "connecting";
      // TODO: invoke('start_singbox')
      setTimeout(() => {
        status = "connected";
        elapsedSec = 0;
        timer = setInterval(() => (elapsedSec += 1), 1000);
      }, 800);
    } else {
      // TODO: invoke('stop_singbox')
      status = "disconnected";
      if (timer) {
        clearInterval(timer);
        timer = null;
      }
      elapsedSec = 0;
    }
  }
</script>

<div class="connect">
  <div class="status">
    <span class="status-dot" data-status={status}></span>
    <span class="status-label">{statusLabel}</span>
  </div>

  <button
    class="power"
    data-status={status}
    onclick={toggle}
    aria-label={status === "connected" ? "Disconnect" : "Connect"}
  >
    <svg viewBox="0 0 64 64" width="56" height="56" aria-hidden="true">
      <path
        d="M22 18a16 16 0 1 0 20 0"
        stroke="currentColor"
        stroke-width="3.5"
        stroke-linecap="round"
        fill="none"
      />
      <line
        x1="32"
        y1="11"
        x2="32"
        y2="30"
        stroke="currentColor"
        stroke-width="3.5"
        stroke-linecap="round"
      />
    </svg>
  </button>

  <div class="action-text">
    {status === "connected" ? "Tap to disconnect" : "Tap to connect"}
  </div>

  {#if status === "connected"}
    <div class="elapsed">{fmt(elapsedSec)}</div>
  {/if}

  <button class="server-pill" onclick={() => goto("/servers")}>
    {#if currentServer}
      <span class="flag">{currentServer.flag}</span>
      <span class="srv-name">{currentServer.name}</span>
      <span class="srv-ping muted">
        {currentServer.pingMs !== null ? `${currentServer.pingMs} ms` : "—"}
      </span>
    {:else}
      <span class="muted">No server selected</span>
    {/if}
    <svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true" class="chev">
      <path d="M9 6l6 6-6 6" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  </button>

  <div class="stats">
    <div class="stat">
      <div class="stat-label muted">Download</div>
      <div class="stat-value">— <span class="unit muted">MB/s</span></div>
    </div>
    <div class="divider"></div>
    <div class="stat">
      <div class="stat-label muted">Upload</div>
      <div class="stat-value">— <span class="unit muted">MB/s</span></div>
    </div>
  </div>
</div>

<style>
  .connect {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 18px;
    padding: 28px 8px 16px;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-muted);
  }
  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-dim);
    transition: background var(--transition);
  }
  .status-dot[data-status="connecting"] {
    background: var(--warn);
    animation: blink 1.1s ease-in-out infinite;
  }
  .status-dot[data-status="connected"] {
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent);
  }
  @keyframes blink {
    0%, 100% { opacity: 0.4; }
    50% { opacity: 1; }
  }
  .status-label {
    font-weight: 600;
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
    margin-top: 8px;
    box-shadow: var(--shadow);
    transition: all var(--transition);
  }
  .power:hover {
    border-color: var(--border-strong);
  }
  .power[data-status="connected"] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-on);
    box-shadow: 0 0 0 6px var(--accent-faint), var(--shadow);
  }
  .power[data-status="connecting"] {
    color: var(--warn);
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    0%, 100% { transform: scale(1); opacity: 0.85; }
    50% { transform: scale(1.04); opacity: 1; }
  }

  .action-text {
    font-size: 13px;
    color: var(--text-muted);
  }

  .elapsed {
    font-variant-numeric: tabular-nums;
    font-size: 22px;
    font-weight: 600;
    color: var(--text);
    margin-top: -10px;
  }

  .server-pill {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    margin-top: 8px;
  }
  .server-pill:hover {
    background: var(--bg-elev-2);
  }
  .flag {
    font-size: 18px;
    line-height: 1;
  }
  .srv-name {
    flex: 1;
    text-align: left;
    font-weight: 500;
  }
  .srv-ping {
    font-variant-numeric: tabular-nums;
    font-size: 12px;
  }
  .chev {
    color: var(--text-muted);
  }

  .stats {
    width: 100%;
    display: grid;
    grid-template-columns: 1fr 1px 1fr;
    align-items: center;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 14px 18px;
  }
  .stat-label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .stat-value {
    font-size: 18px;
    font-weight: 600;
    margin-top: 4px;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    font-size: 11px;
    font-weight: 400;
  }
  .divider {
    height: 28px;
    background: var(--border);
  }
</style>

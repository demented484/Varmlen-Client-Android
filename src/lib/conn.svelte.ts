export type Status = "disconnected" | "connecting" | "connected";

class ConnStore {
  status = $state<Status>("disconnected");
  elapsedSec = $state(0);
  private timer: ReturnType<typeof setInterval> | null = null;

  async toggle(): Promise<void> {
    if (this.status === "disconnected") {
      this.status = "connecting";
      // TODO: invoke('start_singbox', { server_id })
      await new Promise((r) => setTimeout(r, 700));
      this.status = "connected";
      this.elapsedSec = 0;
      this.timer = setInterval(() => (this.elapsedSec += 1), 1000);
    } else {
      // TODO: invoke('stop_singbox')
      this.status = "disconnected";
      if (this.timer) {
        clearInterval(this.timer);
        this.timer = null;
      }
      this.elapsedSec = 0;
    }
  }
}

export const conn = new ConnStore();

export function fmtElapsed(s: number): string {
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${pad(h)}:${pad(m)}:${pad(sec)}`;
}

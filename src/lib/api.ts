import { invoke } from "@tauri-apps/api/core";

export interface VlessServer {
  id: string;
  uuid: string;
  host: string;
  port: number;
  label: string;
  transport: string;
  security: string;
  sni: string | null;
  fingerprint: string | null;
  public_key: string | null;
  short_id: string | null;
  flow: string | null;
  path: string | null;
  mode: string | null;
  packet_encoding: string | null;
  raw_params: Record<string, string>;
}

export interface SubscriptionMeta {
  title: string | null;
  update_interval_hours: number | null;
  upload_bytes: number | null;
  download_bytes: number | null;
  total_bytes: number | null;
  expires_at_unix: number | null;
  support_url: string | null;
}

export interface ImportResult {
  meta: SubscriptionMeta;
  servers: VlessServer[];
}

export function parseVlessUri(uri: string): Promise<VlessServer> {
  return invoke<VlessServer>("parse_vless_uri", { uri });
}

export function parseSubscriptionBody(body: string): Promise<VlessServer[]> {
  return invoke<VlessServer[]>("parse_subscription_body", { body });
}

export function fetchSubscription(url: string): Promise<ImportResult> {
  return invoke<ImportResult>("fetch_subscription", { url });
}

/** TCP RTT in ms. Throws on timeout / unreachable. */
export function pingTcp(host: string, port: number): Promise<number> {
  return invoke<number>("ping_tcp", { host, port });
}

const FLAG_HINTS: Array<[RegExp, string]> = [
  [/finland|finl|\bfi\b|🇫🇮/i,    "🇫🇮"],
  [/sweden|stockholm|\bse\b|🇸🇪/i, "🇸🇪"],
  [/\busa?\b|united states|new york|🇺🇸/i, "🇺🇸"],
  [/germany|deutsch|\bde\b|🇩🇪/i,  "🇩🇪"],
  [/poland|\bpl\b|🇵🇱/i,           "🇵🇱"],
  [/netherland|amsterdam|\bnl\b|🇳🇱/i, "🇳🇱"],
  [/france|paris|\bfr\b|🇫🇷/i,     "🇫🇷"],
  [/japan|tokyo|\bjp\b|🇯🇵/i,       "🇯🇵"],
  [/singapore|\bsg\b|🇸🇬/i,         "🇸🇬"],
  [/uk\b|britain|london|\bgb\b|🇬🇧/i, "🇬🇧"],
  [/turkey|istanbul|\btr\b|🇹🇷/i,   "🇹🇷"],
];

export function guessFlag(label: string): string {
  for (const [re, flag] of FLAG_HINTS) {
    if (re.test(label)) return flag;
  }
  return "🏳️";
}

/** Pretty bytes like 742.3GB / 1.5TB / 0B. */
export function formatBytes(n: number | null): string {
  if (n == null || n <= 0) return "0B";
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)}${units[i]}`;
}

/** Format unix seconds as DD.MM.YYYY for the expires badge. */
export function formatExpires(unix: number | null): string | null {
  if (!unix || unix <= 0) return null;
  const d = new Date(unix * 1000);
  if (!Number.isFinite(d.getTime())) return null;
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()}`;
}

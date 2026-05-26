import { invoke } from "@tauri-apps/api/core";

export interface VlessServer {
  id: string;
  /** "vless" | "trojan" | "shadowsocks" | "vmess" */
  protocol: string;
  uuid: string;
  password: string | null;
  method: string | null;
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
  web_page_url: string | null;
}

export interface ImportResult {
  meta: SubscriptionMeta;
  servers: VlessServer[];
  description: string | null;
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

/** TCP-connect RTT to host:port, in ms — the real wire latency (same as ICMP
 *  ping for a healthy server). Throws on timeout / unreachable. */
export function pingTcp(host: string, port: number): Promise<number> {
  return invoke<number>("ping_tcp", { host, port });
}

export interface InstalledApp {
  /** Binary / process name used to match the running app. */
  id: string;
  /** Display name from the desktop entry. */
  name: string;
  /** Icon as a `data:image/...` URI, or null when none was resolved. */
  icon: string | null;
}

/** Installed desktop apps, parsed from the system's `.desktop` entries. */
export function listInstalledApps(): Promise<InstalledApp[]> {
  return invoke<InstalledApp[]>("list_installed_apps");
}

/** Open the system file picker (XDG portal → native DE dialog with search).
 *  Returns the chosen path, or null if cancelled. */
export function pickFile(): Promise<string | null> {
  return invoke<string | null>("pick_file");
}

/** Build an app entry from a user-picked file: a `.desktop` file is parsed
 *  (name / exec / icon), any other file is treated as the binary. */
export function appFromFile(path: string): Promise<InstalledApp | null> {
  return invoke<InstalledApp | null>("app_from_file", { path });
}

export interface CoreInfo {
  /** Installed sing-box version, or null when not installed. */
  installed: string | null;
  /** Latest available version, or null when the check failed. */
  latest: string | null;
  has_update: boolean;
}

/** Installed vs latest sing-box core version (queries GitHub releases). */
export function coreInfo(): Promise<CoreInfo> {
  return invoke<CoreInfo>("core_info");
}

/** Install a specific sing-box version (or latest when `version` is null). */
export function coreInstall(version: string | null = null): Promise<string> {
  return invoke<string>("core_install", { version });
}

export interface CoreRelease {
  tag: string;
  name: string;
  date: string | null;
  prerelease: boolean;
}

/** Recent sing-box releases (newest first) for the version picker. */
export function listCoreReleases(): Promise<CoreRelease[]> {
  return invoke<CoreRelease[]>("list_core_releases");
}

/** Enabled split-tunnel selection passed to the connect command. */
export interface SplitInput {
  apps_mode: string;
  sites_mode: string;
  apps: string[];
  sites: string[];
}

export interface HelperResponse {
  ok: boolean;
  state: "connected" | "disconnected" | "unknown" | string;
  pid: number | null;
  error: string | null;
}

/** Connect in the given mode: "tun" (full system, via the root helper) or
 *  "proxy" (local SOCKS/HTTP, run as the user — no root). */
export function vpnConnect(
  server: VlessServer,
  split: SplitInput,
  mode: "tun" | "proxy",
  killswitch: boolean,
  allowLan: boolean,
): Promise<HelperResponse> {
  return invoke<HelperResponse>("vpn_connect", { server, split, mode, killswitch, allowLan });
}

export function vpnDisconnect(): Promise<HelperResponse> {
  return invoke<HelperResponse>("vpn_disconnect");
}

export function vpnStatus(): Promise<HelperResponse> {
  return invoke<HelperResponse>("vpn_status");
}

/** Whether the privileged helper is installed and reachable. */
export function helperInstalled(): Promise<boolean> {
  return invoke<boolean>("helper_installed");
}

/** Install the privileged helper via a one-time pkexec (polkit) prompt. */
export function installHelper(): Promise<void> {
  return invoke<void>("install_helper");
}

/** One-time migration: read any prior dev-origin localStorage (subs, split,
 *  settings, …) so they aren't lost when the release build switches origin.
 *  Throws on error — frontend logs to console if migration can't run. */
export function readLegacyStorage(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>("read_legacy_storage");
}

/** The single leading emoji cluster at the start of a label: a country flag
 *  (two regional indicators) or one pictographic emoji (📶 …) with its
 *  modifiers / ZWJ sequence / variation selector. Only the FIRST one. */
const LEADING_EMOJI =
  /^(?:\p{Regional_Indicator}\p{Regional_Indicator}|\p{Extended_Pictographic})(?:️|\p{Emoji_Modifier}|‍\p{Extended_Pictographic}️?)*/u;

/** Split a server label into its leading emoji icon (just the first one) and
 *  the remaining text, so the icon renders in its own slot and isn't duplicated
 *  in the name. Panels prefix a country flag (or a 📶-style marker); we use
 *  whatever they send rather than guessing from the text. */
export function splitLabelEmoji(label: string): { icon: string; name: string } {
  const m = label.match(LEADING_EMOJI);
  if (!m) return { icon: "", name: label.trim() };
  return { icon: m[0], name: label.slice(m[0].length).trim() };
}

/** Server name with the leading emoji icon removed. */
export function stripLeadingFlag(label: string): string {
  return splitLabelEmoji(label).name;
}

/** The icon (flag or other leading emoji) for a server, or "" when none. */
export function flagFor(label: string): string {
  return splitLabelEmoji(label).icon;
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

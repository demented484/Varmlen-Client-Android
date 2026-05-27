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

export interface InstalledVersion {
  /** Version tag like "1.13.0" (no leading "v"). */
  tag: string;
  /** True iff this is the currently active version. */
  active: boolean;
}

export interface CoreInfo {
  /** Locally cached versions, newest first. */
  installed: InstalledVersion[];
  /** Active tag, or null when no version is installed. */
  active: string | null;
  /** Latest version on GitHub, or null when the check failed. */
  latest: string | null;
  /** True iff `latest` is newer than `active` (or no version is active). */
  has_update: boolean;
}

/** Installed/active vs latest sing-box core version (queries GitHub releases). */
export function coreInfo(): Promise<CoreInfo> {
  return invoke<CoreInfo>("core_info");
}

/** Download a specific sing-box version (or latest when `version` is null)
 *  into the local cache. Emits `core://progress` events while running.
 *  First-install case auto-activates the new version. */
export function coreInstall(version: string | null = null): Promise<string> {
  return invoke<string>("core_install", { version });
}

/** Switch the active version to one that's already downloaded; pushes the
 *  binary to the privileged helper so TUN mode picks it up too. */
export function coreActivate(tag: string): Promise<void> {
  return invoke<void>("core_activate", { tag });
}

/** Delete a cached version from disk. Refuses to delete the active one. */
export function coreUninstall(tag: string): Promise<void> {
  return invoke<void>("core_uninstall", { tag });
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

export interface CoreProgress {
  /** Tag the progress refers to. */
  tag: string;
  /** Bytes downloaded so far. */
  downloaded: number;
  /** Total expected bytes (0 when the server didn't send Content-Length). */
  total: number;
  /** Bytes per second over the last sample window. */
  speed_bps: number;
}


/** Enabled split-tunnel selection passed to the connect command. */
export interface SplitInput {
  /** "selective" (whitelist: only listed apps+sites via VPN) or
   *  "general"   (blacklist: all via VPN except listed apps+sites). */
  mode: string;
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

/** TCP-connect RTT to host:port in ms. Source-bound to the user's physical
 *  interface so the result reflects the real network even when the VPN tunnel
 *  is active. Rejects on DNS/timeout/refused. */
export function tcpPingHost(host: string, port: number, timeoutMs = 2500): Promise<number> {
  return invoke<number>("tcp_ping_host", { host, port, timeoutMs });
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

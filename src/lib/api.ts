import { invoke } from "@tauri-apps/api/core";

/** Mirrors `VlessServer` in src-tauri/src/subscription.rs. */
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

export function parseVlessUri(uri: string): Promise<VlessServer> {
  return invoke<VlessServer>("parse_vless_uri", { uri });
}

export function parseSubscriptionBody(body: string): Promise<VlessServer[]> {
  return invoke<VlessServer[]>("parse_subscription_body", { body });
}

export function fetchSubscription(url: string): Promise<VlessServer[]> {
  return invoke<VlessServer[]>("fetch_subscription", { url });
}

/** Best-effort emoji flag from common 2-letter country hints in the label. */
export function guessFlag(label: string): string {
  const hints: Array<[RegExp, string]> = [
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
  for (const [re, flag] of hints) {
    if (re.test(label)) return flag;
  }
  return "🏳️";
}

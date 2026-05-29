//! Proxy URI and subscription parser.
//!
//! Supports the schemes commonly found in subscriptions:
//! - `vless://<uuid>@<host>:<port>?<params>#<label>`
//! - `trojan://<password>@<host>:<port>?<params>#<label>`
//! - `ss://<base64(method:password)>@<host>:<port>#<label>` (and legacy forms)
//! - `vmess://<base64-json>`
//!
//! Subscription body: either plaintext (one URI per line) or base64-encoded
//! plaintext. Whitespace-only lines and comment lines (`#…`) are ignored.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid URI: {0}")]
    InvalidUri(String),
    #[error("unsupported scheme '{0}'")]
    UnsupportedScheme(String),
    #[error("missing credentials")]
    MissingCredentials,
    #[error("missing host")]
    MissingHost,
    #[error("missing port")]
    MissingPort,
}

fn default_protocol() -> String {
    "vless".to_string()
}

/// Deserialize `protocol`, tolerating a missing key, an explicit `null`, or an
/// empty string from subscriptions persisted before multi-protocol support —
/// all of which mean the legacy default, vless.
fn de_protocol<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(d)?;
    Ok(opt.filter(|s| !s.is_empty()).unwrap_or_else(default_protocol))
}

/// A single VPN endpoint parsed from a proxy URI. The struct keeps its
/// historical name; `protocol` distinguishes vless / trojan / shadowsocks /
/// vmess, and credential fields are filled per-protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlessServer {
    pub id: String,
    /// Protocol: "vless" | "trojan" | "shadowsocks" | "vmess". Tolerant of
    /// missing/null/empty (legacy data) → defaults to "vless".
    #[serde(default = "default_protocol", deserialize_with = "de_protocol")]
    pub protocol: String,
    /// VLESS/VMess UUID, or Trojan password. Empty for Shadowsocks.
    pub uuid: String,
    /// Shadowsocks/Trojan password (Shadowsocks keeps it separate from method).
    #[serde(default)]
    pub password: Option<String>,
    /// Shadowsocks cipher method.
    #[serde(default)]
    pub method: Option<String>,
    pub host: String,
    pub port: u16,
    pub label: String,
    pub transport: String,
    pub security: String,
    #[serde(default)]
    pub sni: Option<String>,
    #[serde(default)]
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub public_key: Option<String>,
    #[serde(default)]
    pub short_id: Option<String>,
    #[serde(default)]
    pub flow: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub packet_encoding: Option<String>,
    #[serde(default)]
    pub raw_params: HashMap<String, String>,
}

impl VlessServer {
    fn base(protocol: &str, host: String, port: u16, label: String) -> Self {
        VlessServer {
            id: format!("{host}_{port}"),
            protocol: protocol.to_string(),
            uuid: String::new(),
            password: None,
            method: None,
            host,
            port,
            label,
            transport: "tcp".to_string(),
            security: "none".to_string(),
            sni: None,
            fingerprint: None,
            public_key: None,
            short_id: None,
            flow: None,
            path: None,
            mode: None,
            packet_encoding: None,
            raw_params: HashMap::new(),
        }
    }
}

/// Server-side subscription metadata parsed from HTTP response headers.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SubscriptionMeta {
    /// `Profile-Title` header — used as the subscription display name.
    pub title: Option<String>,
    /// `Profile-Update-Interval` (hours).
    pub update_interval_hours: Option<u32>,
    /// `Subscription-Userinfo`: upload bytes used.
    pub upload_bytes: Option<u64>,
    /// `Subscription-Userinfo`: download bytes used.
    pub download_bytes: Option<u64>,
    /// `Subscription-Userinfo`: total quota in bytes. 0 means unlimited.
    pub total_bytes: Option<u64>,
    /// `Subscription-Userinfo`: expiry as unix seconds.
    pub expires_at_unix: Option<i64>,
    /// `Support-Url` — a human support contact (channel / chat).
    pub support_url: Option<String>,
    /// `Profile-Web-Page-Url` — the provider's bot / web page.
    pub web_page_url: Option<String>,
}

/// Bundled result of an import: headers + parsed servers + free-text
/// description extracted from leading `# …` comments in the body.
#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub meta: SubscriptionMeta,
    pub servers: Vec<VlessServer>,
    pub description: Option<String>,
}

/// Metadata keys that panels (Marzban / Happ-style) inline into the body as
/// `#key: value` lines, duplicating the HTTP response headers. These are NOT
/// human-readable description — they carry client settings — so we route them
/// to metadata and keep them out of the shown description.
fn is_meta_key(key: &str) -> bool {
    matches!(
        key,
        "profile-title"
            | "profile-update-interval"
            | "support-url"
            | "profile-web-page-url"
            | "subscription-userinfo"
            | "announce"
            | "hide-settings"
            | "subscriptions-collapse"
            | "subscriptions-expand-now"
            | "encrypted-subscription"
            | "allow-insecure"
            | "subscription-ping-onopen-enabled"
            | "mux-enable"
            | "mux-tcp-connections"
            | "mux-xudp-connections"
            | "mux-quic"
            | "routing"
            | "dns"
    )
}

/// Parse the leading comment block of a subscription body. Panels prepend two
/// very different things there:
///   1. `#key: value` lines duplicating the HTTP headers (profile-title,
///      subscription-userinfo, mux-*, …) — collected into `headers`.
///   2. Free-text lines (a real human note, or a base64 `announce` banner) —
///      joined into `description`.
/// Stops at the first non-comment, non-blank line (the first proxy URI).
pub fn parse_body_meta(body: &str) -> (std::collections::HashMap<String, String>, Option<String>) {
    let text = decode_body(body);
    let mut headers = std::collections::HashMap::new();
    let mut desc_lines = Vec::<String>::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            if headers.is_empty() && desc_lines.is_empty() {
                continue; // skip leading blank lines
            }
            break; // blank line ends the comment block
        }
        let Some(rest) = line.strip_prefix('#') else {
            break; // first real (non-comment) line — stop
        };
        let rest = rest.trim();
        // `#key: value` where key looks like a metadata field → header, not desc.
        if let Some((k, v)) = rest.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            if is_meta_key(&key) {
                headers.insert(key, v.trim().to_string());
                continue;
            }
        }
        desc_lines.push(rest.to_string());
    }
    let description = if desc_lines.is_empty() {
        None
    } else {
        Some(desc_lines.join("\n"))
    };
    (headers, description)
}

/// Backwards-compatible wrapper: just the free-text description part.
pub fn extract_description(body: &str) -> Option<String> {
    parse_body_meta(body).1
}

/// Schemes we know how to parse.
pub fn is_supported_uri(line: &str) -> bool {
    let l = line.trim();
    l.starts_with("vless://")
        || l.starts_with("trojan://")
        || l.starts_with("ss://")
        || l.starts_with("vmess://")
}

/// Parse any supported proxy URI, dispatching on its scheme.
pub fn parse_proxy_uri(uri: &str) -> Result<VlessServer, ParseError> {
    let uri = uri.trim();
    if uri.starts_with("vless://") {
        parse_vless(uri)
    } else if uri.starts_with("trojan://") {
        parse_trojan(uri)
    } else if uri.starts_with("ss://") {
        parse_shadowsocks(uri)
    } else if uri.starts_with("vmess://") {
        parse_vmess(uri)
    } else {
        let scheme = uri.split("://").next().unwrap_or(uri);
        Err(ParseError::UnsupportedScheme(scheme.to_string()))
    }
}

fn label_from(fragment: Option<&str>, host: &str, port: u16) -> String {
    match fragment {
        Some(f) if !f.is_empty() => percent_decode(f),
        _ => format!("{host}:{port}"),
    }
}

/// Parse a single `vless://` URI.
pub fn parse_vless(uri: &str) -> Result<VlessServer, ParseError> {
    let url = Url::parse(uri.trim()).map_err(|e| ParseError::InvalidUri(e.to_string()))?;
    if url.scheme() != "vless" {
        return Err(ParseError::UnsupportedScheme(url.scheme().to_string()));
    }

    let uuid = url.username();
    if uuid.is_empty() {
        return Err(ParseError::MissingCredentials);
    }
    let uuid = percent_decode(uuid);

    let host = url.host_str().ok_or(ParseError::MissingHost)?.to_string();
    let port = url.port().ok_or(ParseError::MissingPort)?;

    let params: HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let mut s = VlessServer::base("vless", host.clone(), port, label_from(url.fragment(), &host, port));
    s.uuid = uuid;
    s.transport = params.get("type").cloned().unwrap_or_else(|| "tcp".to_string());
    s.security = params.get("security").cloned().unwrap_or_else(|| "none".to_string());
    s.sni = params.get("sni").cloned();
    s.fingerprint = params.get("fp").cloned();
    s.public_key = params.get("pbk").cloned();
    s.short_id = params.get("sid").cloned();
    s.flow = params.get("flow").cloned();
    s.path = params.get("path").cloned();
    s.mode = params.get("mode").cloned();
    s.packet_encoding = params.get("packetEncoding").cloned();
    s.raw_params = params;
    Ok(s)
}

/// Parse a single `trojan://<password>@<host>:<port>?<params>#<label>` URI.
pub fn parse_trojan(uri: &str) -> Result<VlessServer, ParseError> {
    let url = Url::parse(uri.trim()).map_err(|e| ParseError::InvalidUri(e.to_string()))?;
    if url.scheme() != "trojan" {
        return Err(ParseError::UnsupportedScheme(url.scheme().to_string()));
    }
    let password = percent_decode(url.username());
    if password.is_empty() {
        return Err(ParseError::MissingCredentials);
    }
    let host = url.host_str().ok_or(ParseError::MissingHost)?.to_string();
    let port = url.port().ok_or(ParseError::MissingPort)?;
    let params: HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let mut s = VlessServer::base("trojan", host.clone(), port, label_from(url.fragment(), &host, port));
    s.uuid = password.clone();
    s.password = Some(password);
    s.transport = params.get("type").cloned().unwrap_or_else(|| "tcp".to_string());
    s.security = params.get("security").cloned().unwrap_or_else(|| "tls".to_string());
    s.sni = params.get("sni").cloned();
    s.fingerprint = params.get("fp").cloned();
    s.path = params.get("path").cloned();
    s.raw_params = params;
    Ok(s)
}

/// Parse a Shadowsocks `ss://` URI. Handles SIP002 (base64 userinfo) and the
/// legacy fully-base64 form, with or without a trailing `?plugin=…`.
pub fn parse_shadowsocks(uri: &str) -> Result<VlessServer, ParseError> {
    let rest = uri.trim().strip_prefix("ss://").unwrap_or("");
    // Split off the #fragment label.
    let (main, fragment) = match rest.split_once('#') {
        Some((m, f)) => (m, Some(f)),
        None => (rest, None),
    };
    // Drop any ?plugin=… query.
    let main = main.split('?').next().unwrap_or(main);

    // Two layouts: "userinfo@host:port" or base64("method:password@host:port").
    let (creds, hostport) = if let Some((u, hp)) = main.rsplit_once('@') {
        // userinfo may itself be base64(method:password).
        let decoded = b64_decode_str(u).unwrap_or_else(|| percent_decode(u));
        (decoded, hp.to_string())
    } else {
        // Whole thing is base64.
        let decoded = b64_decode_str(main).ok_or_else(|| ParseError::InvalidUri("ss base64".into()))?;
        let (u, hp) = decoded.rsplit_once('@').ok_or(ParseError::MissingHost)?;
        (u.to_string(), hp.to_string())
    };

    let (method, password) = creds
        .split_once(':')
        .ok_or(ParseError::MissingCredentials)?;
    let (host, port_str) = hostport.rsplit_once(':').ok_or(ParseError::MissingPort)?;
    let port: u16 = port_str.parse().map_err(|_| ParseError::MissingPort)?;
    let host = host.trim_matches(['[', ']']).to_string();

    let mut s = VlessServer::base("shadowsocks", host.clone(), port, label_from(fragment, &host, port));
    s.method = Some(method.to_string());
    s.password = Some(password.to_string());
    Ok(s)
}

/// Parse a `vmess://<base64-json>` URI (the common v2rayN JSON form).
pub fn parse_vmess(uri: &str) -> Result<VlessServer, ParseError> {
    let payload = uri.trim().strip_prefix("vmess://").unwrap_or("");
    let json = b64_decode_str(payload).ok_or_else(|| ParseError::InvalidUri("vmess base64".into()))?;
    let v: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| ParseError::InvalidUri(e.to_string()))?;

    let host = v.get("add").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if host.is_empty() {
        return Err(ParseError::MissingHost);
    }
    // port may be a number or a string.
    let port: u16 = match v.get("port") {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(0) as u16,
        Some(serde_json::Value::String(st)) => st.parse().unwrap_or(0),
        _ => 0,
    };
    if port == 0 {
        return Err(ParseError::MissingPort);
    }
    let uuid = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if uuid.is_empty() {
        return Err(ParseError::MissingCredentials);
    }
    let ps = v.get("ps").and_then(|x| x.as_str()).map(|s| s.to_string());

    let mut s = VlessServer::base("vmess", host.clone(), port, ps.unwrap_or_else(|| format!("{host}:{port}")));
    s.uuid = uuid;
    s.transport = v.get("net").and_then(|x| x.as_str()).unwrap_or("tcp").to_string();
    s.security = match v.get("tls").and_then(|x| x.as_str()) {
        Some("tls") => "tls".to_string(),
        _ => "none".to_string(),
    };
    s.sni = v.get("sni").and_then(|x| x.as_str()).map(|s| s.to_string());
    s.path = v.get("path").and_then(|x| x.as_str()).map(|s| s.to_string());
    Ok(s)
}

/// Parse a subscription body: a list of proxy URIs (plaintext or base64).
/// Lines with unsupported schemes are skipped.
pub fn parse_subscription(body: &str) -> Vec<VlessServer> {
    let text = decode_body(body);
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || !is_supported_uri(trimmed) {
                return None;
            }
            parse_proxy_uri(trimmed).ok()
        })
        .collect()
}

/// Best-effort base64 decode (standard / url-safe, padded or not) to UTF-8.
fn b64_decode_str(s: &str) -> Option<String> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    for engine in [
        &base64::engine::general_purpose::STANDARD,
        &base64::engine::general_purpose::URL_SAFE,
        &base64::engine::general_purpose::STANDARD_NO_PAD,
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ] {
        if let Ok(bytes) = engine.decode(compact.as_bytes()) {
            if let Ok(text) = String::from_utf8(bytes) {
                return Some(text);
            }
        }
    }
    None
}

fn decode_body(body: &str) -> String {
    let compact: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    // Already plaintext if it carries any proxy URI scheme.
    if ["vless://", "trojan://", "ss://", "vmess://"]
        .iter()
        .any(|s| compact.contains(s))
    {
        return body.to_string();
    }
    for engine in [
        &base64::engine::general_purpose::STANDARD,
        &base64::engine::general_purpose::URL_SAFE,
        &base64::engine::general_purpose::STANDARD_NO_PAD,
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ] {
        if let Ok(bytes) = engine.decode(compact.as_bytes()) {
            if let Ok(s) = String::from_utf8(bytes) {
                return s;
            }
        }
    }
    body.to_string()
}

fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .into_owned()
}

/// Decode a header value that some panels base64-encode (e.g. a non-ASCII
/// `Profile-Title` or `announce`). The convention is a `base64:` prefix, but a
/// few panels send raw base64 with no prefix, so fall back to a best-effort
/// decode that only replaces the value when the result is valid UTF-8.
pub fn decode_maybe_b64(value: &str) -> String {
    let trimmed = value.trim();
    let payload = trimmed
        .strip_prefix("base64:")
        .or_else(|| trimmed.strip_prefix("Base64:"))
        .map(str::trim);

    // With an explicit prefix we always try to decode; without one we only
    // attempt it when the string can't already be meaningful text.
    let (candidate, explicit) = match payload {
        Some(p) => (p, true),
        None => (trimmed, false),
    };

    for engine in [
        &base64::engine::general_purpose::STANDARD,
        &base64::engine::general_purpose::URL_SAFE,
        &base64::engine::general_purpose::STANDARD_NO_PAD,
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ] {
        if let Ok(bytes) = engine.decode(candidate.as_bytes()) {
            if let Ok(s) = String::from_utf8(bytes) {
                let s = s.trim().to_string();
                // Without a prefix, guard against false positives: only accept a
                // decode that yields printable text different from the input.
                if explicit || (!s.is_empty() && s != trimmed && s.chars().all(|c| !c.is_control())) {
                    return s;
                }
            }
        }
    }
    trimmed.to_string()
}

/// Extract subscription metadata from response headers.
///
/// Header names are lowercase ASCII. `Subscription-Userinfo` looks like
/// `upload=0; download=0; total=10737418240; expire=1781461695`.
pub fn parse_headers<F>(get: F) -> SubscriptionMeta
where
    F: Fn(&str) -> Option<String>,
{
    let mut meta = SubscriptionMeta::default();
    meta.title = get("profile-title")
        .map(|s| decode_maybe_b64(&s))
        .filter(|s| !s.is_empty());
    meta.update_interval_hours = get("profile-update-interval")
        .and_then(|s| s.trim().parse().ok());
    meta.support_url = get("support-url")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    meta.web_page_url = get("profile-web-page-url")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if let Some(info) = get("subscription-userinfo") {
        for kv in info.split(';') {
            let kv = kv.trim();
            if let Some((k, v)) = kv.split_once('=') {
                let v = v.trim();
                match k.trim() {
                    "upload"   => meta.upload_bytes   = v.parse().ok(),
                    "download" => meta.download_bytes = v.parse().ok(),
                    "total"    => meta.total_bytes    = v.parse().ok(),
                    "expire"   => meta.expires_at_unix = v.parse().ok(),
                    _ => {}
                }
            }
        }
    }
    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_vless_reality_xhttp() {
        let uri = "vless://3f7e7d8c-1234-5678-9abc-def012345678@89.125.181.236:443?type=xhttp&security=reality&sni=gateway.icloud.com&fp=chrome&pbk=ABC&sid=DEAD&path=/fi-exp-xh-1673aadd&mode=packet-up&packetEncoding=xudp#Finland%20Exp";
        let s = parse_vless(uri).expect("parse");
        assert_eq!(s.uuid, "3f7e7d8c-1234-5678-9abc-def012345678");
        assert_eq!(s.host, "89.125.181.236");
        assert_eq!(s.port, 443);
        assert_eq!(s.label, "Finland Exp");
        assert_eq!(s.transport, "xhttp");
        assert_eq!(s.security, "reality");
        assert_eq!(s.sni.as_deref(), Some("gateway.icloud.com"));
        assert_eq!(s.path.as_deref(), Some("/fi-exp-xh-1673aadd"));
    }

    #[test]
    fn rejects_non_vless() {
        let r = parse_vless("vmess://AAAA@1.2.3.4:443");
        assert!(matches!(r, Err(ParseError::UnsupportedScheme(_))));
    }

    #[test]
    fn parses_plaintext_subscription() {
        let body = "# c\nvless://a@h-a:443?type=tcp&security=reality#A\nvless://b@h-b:443?type=xhttp&security=reality#B\ngarbage";
        let v = parse_subscription(body);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn inline_headers_are_meta_not_description() {
        // Marzban / Happ-style: panel inlines #key: value lines at the top.
        // They must go to metadata, NOT be shown as the description.
        let body = concat!(
            "#profile-title: KurtaVPN\n",
            "#profile-update-interval: 3\n",
            "#subscription-userinfo: upload=10; download=20; total=0; expire=1780236569\n",
            "#mux-enable: 1\n",
            "#announce: base64:0J/RgNC40LLQtdGC\n", // "Привет"
            "vless://a@h-a:443?type=tcp&security=reality#A\n",
        );
        let (headers, desc) = parse_body_meta(body);
        assert_eq!(headers.get("profile-title").map(String::as_str), Some("KurtaVPN"));
        assert_eq!(headers.get("mux-enable").map(String::as_str), Some("1"));
        assert!(headers.contains_key("subscription-userinfo"));
        // None of the #key: value lines leak into description.
        assert!(desc.is_none(), "description should be empty, got {desc:?}");

        // And the inline userinfo parses into meta via the same path headers do.
        let m = parse_headers(|name| headers.get(name).cloned());
        assert_eq!(m.total_bytes, Some(0));
        assert_eq!(m.upload_bytes, Some(10));
        assert_eq!(m.expires_at_unix, Some(1780236569));
    }

    #[test]
    fn freetext_comment_is_description() {
        let body = "# Обходы внизу списка\nvless://a@h-a:443?type=tcp&security=reality#A";
        let (headers, desc) = parse_body_meta(body);
        assert!(headers.is_empty());
        assert_eq!(desc.as_deref(), Some("Обходы внизу списка"));
    }

    #[test]
    fn parses_base64_subscription() {
        let plain = "vless://a@h-a:443?type=tcp&security=reality#A\nvless://b@h-b:443?type=xhttp&security=reality#B";
        let b64 = base64::engine::general_purpose::STANDARD.encode(plain.as_bytes());
        let v = parse_subscription(&b64);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn parses_shadowsocks_sip002() {
        // base64("chacha20-ietf-poly1305:secretpass")
        let creds = base64::engine::general_purpose::STANDARD.encode("chacha20-ietf-poly1305:secretpass");
        let uri = format!("ss://{creds}@45.144.52.226:2060#%F0%9F%87%AB%F0%9F%87%AE%20FI");
        let s = parse_shadowsocks(&uri).unwrap();
        assert_eq!(s.protocol, "shadowsocks");
        assert_eq!(s.host, "45.144.52.226");
        assert_eq!(s.port, 2060);
        assert_eq!(s.method.as_deref(), Some("chacha20-ietf-poly1305"));
        assert_eq!(s.password.as_deref(), Some("secretpass"));
        assert!(s.label.contains("FI"));
    }

    #[test]
    fn parses_trojan() {
        let s = parse_trojan("trojan://pass123@h.example.com:443?sni=h.example.com#Trojan").unwrap();
        assert_eq!(s.protocol, "trojan");
        assert_eq!(s.host, "h.example.com");
        assert_eq!(s.port, 443);
        assert_eq!(s.password.as_deref(), Some("pass123"));
        assert_eq!(s.sni.as_deref(), Some("h.example.com"));
    }

    #[test]
    fn parses_vmess() {
        let json = r#"{"v":"2","ps":"My VMess","add":"1.2.3.4","port":"443","id":"uuid-1234","net":"ws","tls":"tls","path":"/p"}"#;
        let b64 = base64::engine::general_purpose::STANDARD.encode(json);
        let s = parse_vmess(&format!("vmess://{b64}")).unwrap();
        assert_eq!(s.protocol, "vmess");
        assert_eq!(s.host, "1.2.3.4");
        assert_eq!(s.port, 443);
        assert_eq!(s.uuid, "uuid-1234");
        assert_eq!(s.transport, "ws");
        assert_eq!(s.security, "tls");
        assert_eq!(s.label, "My VMess");
    }

    #[test]
    fn subscription_keeps_mixed_protocols() {
        let creds = base64::engine::general_purpose::STANDARD.encode("aes-256-gcm:pw");
        let body = format!(
            "vless://a@h-a:443?type=tcp&security=reality#A\nss://{creds}@h-b:2060#B\ntrojan://p@h-c:443#C\nfoobar://nope"
        );
        let v = parse_subscription(&body);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].protocol, "vless");
        assert_eq!(v[1].protocol, "shadowsocks");
        assert_eq!(v[2].protocol, "trojan");
    }

    #[test]
    fn parses_meta_headers() {
        let m = parse_headers(|name| match name {
            "profile-title" => Some("AegisVPN".into()),
            "profile-update-interval" => Some("12".into()),
            "subscription-userinfo" => Some("upload=10; download=200; total=1099511627776; expire=1781461695".into()),
            "support-url" => Some("https://t.me/x_bot".into()),
            _ => None,
        });
        assert_eq!(m.title.as_deref(), Some("AegisVPN"));
        assert_eq!(m.update_interval_hours, Some(12));
        assert_eq!(m.upload_bytes, Some(10));
        assert_eq!(m.download_bytes, Some(200));
        assert_eq!(m.total_bytes, Some(1_099_511_627_776));
        assert_eq!(m.expires_at_unix, Some(1_781_461_695));
        assert_eq!(m.support_url.as_deref(), Some("https://t.me/x_bot"));
    }

    #[test]
    fn meta_missing_headers_yields_defaults() {
        let m = parse_headers(|_| None);
        assert!(m.title.is_none());
        assert!(m.total_bytes.is_none());
    }

    #[test]
    fn decodes_base64_prefixed_title() {
        let b64 = base64::engine::general_purpose::STANDARD.encode("Borealis VPS".as_bytes());
        let m = parse_headers(|name| match name {
            "profile-title" => Some(format!("base64:{b64}")),
            _ => None,
        });
        assert_eq!(m.title.as_deref(), Some("Borealis VPS"));
    }

    #[test]
    fn plain_title_is_left_untouched() {
        let m = parse_headers(|name| match name {
            "profile-title" => Some("AegisVPN".into()),
            _ => None,
        });
        assert_eq!(m.title.as_deref(), Some("AegisVPN"));
    }
}

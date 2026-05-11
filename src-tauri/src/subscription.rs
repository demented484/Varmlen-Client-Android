//! VLESS URI and subscription parser.
//!
//! - `vless://<uuid>@<host>:<port>?<params>#<label>`
//! - subscription body: either plaintext (one URI per line) or base64-encoded
//!   plaintext. Whitespace-only lines and comment lines (`#…`) are ignored.

use base64::Engine;
use serde::Serialize;
use std::collections::HashMap;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid URI: {0}")]
    InvalidUri(String),
    #[error("only vless:// is supported (got '{0}')")]
    UnsupportedScheme(String),
    #[error("missing UUID")]
    MissingUuid,
    #[error("missing host")]
    MissingHost,
    #[error("missing port")]
    MissingPort,
}

/// A single VPN endpoint parsed from a VLESS URI.
#[derive(Debug, Clone, Serialize)]
pub struct VlessServer {
    pub id: String,
    pub uuid: String,
    pub host: String,
    pub port: u16,
    pub label: String,
    pub transport: String,
    pub security: String,
    pub sni: Option<String>,
    pub fingerprint: Option<String>,
    pub public_key: Option<String>,
    pub short_id: Option<String>,
    pub flow: Option<String>,
    pub path: Option<String>,
    pub mode: Option<String>,
    pub packet_encoding: Option<String>,
    pub raw_params: HashMap<String, String>,
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
    /// `Support-Url` or `Profile-Web-Page-Url`.
    pub support_url: Option<String>,
}

/// Bundled result of an import: headers + parsed servers.
#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub meta: SubscriptionMeta,
    pub servers: Vec<VlessServer>,
}

/// Parse a single `vless://` URI.
pub fn parse_vless(uri: &str) -> Result<VlessServer, ParseError> {
    let url = Url::parse(uri.trim()).map_err(|e| ParseError::InvalidUri(e.to_string()))?;
    if url.scheme() != "vless" {
        return Err(ParseError::UnsupportedScheme(url.scheme().to_string()));
    }

    let uuid = url.username();
    if uuid.is_empty() {
        return Err(ParseError::MissingUuid);
    }
    let uuid = percent_decode(uuid);

    let host = url.host_str().ok_or(ParseError::MissingHost)?.to_string();
    let port = url.port().ok_or(ParseError::MissingPort)?;

    let params: HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let transport = params
        .get("type")
        .cloned()
        .unwrap_or_else(|| "tcp".to_string());
    let security = params
        .get("security")
        .cloned()
        .unwrap_or_else(|| "none".to_string());

    let fragment = url.fragment().unwrap_or("").to_string();
    let label = if fragment.is_empty() {
        format!("{}:{}", host, port)
    } else {
        percent_decode(&fragment)
    };

    Ok(VlessServer {
        id: format!("{}_{}", host, port),
        uuid,
        host,
        port,
        label,
        transport,
        security,
        sni: params.get("sni").cloned(),
        fingerprint: params.get("fp").cloned(),
        public_key: params.get("pbk").cloned(),
        short_id: params.get("sid").cloned(),
        flow: params.get("flow").cloned(),
        path: params.get("path").cloned(),
        mode: params.get("mode").cloned(),
        packet_encoding: params.get("packetEncoding").cloned(),
        raw_params: params,
    })
}

/// Parse a subscription body: a list of URIs (plaintext or base64).
pub fn parse_subscription(body: &str) -> Vec<VlessServer> {
    let text = decode_body(body);
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            if !trimmed.starts_with("vless://") {
                return None;
            }
            parse_vless(trimmed).ok()
        })
        .collect()
}

fn decode_body(body: &str) -> String {
    let compact: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.contains("vless://") {
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

/// Extract subscription metadata from response headers.
///
/// Header names are lowercase ASCII. `Subscription-Userinfo` looks like
/// `upload=0; download=0; total=10737418240; expire=1781461695`.
pub fn parse_headers<F>(get: F) -> SubscriptionMeta
where
    F: Fn(&str) -> Option<String>,
{
    let mut meta = SubscriptionMeta::default();
    meta.title = get("profile-title").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    meta.update_interval_hours = get("profile-update-interval")
        .and_then(|s| s.trim().parse().ok());
    meta.support_url = get("support-url")
        .or_else(|| get("profile-web-page-url"))
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
    fn parses_base64_subscription() {
        let plain = "vless://a@h-a:443?type=tcp&security=reality#A\nvless://b@h-b:443?type=xhttp&security=reality#B";
        let b64 = base64::engine::general_purpose::STANDARD.encode(plain.as_bytes());
        let v = parse_subscription(&b64);
        assert_eq!(v.len(), 2);
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
}

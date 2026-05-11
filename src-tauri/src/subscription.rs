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
    /// `tcp` | `xhttp` | `ws` | ... (raw value from `type=`).
    pub transport: String,
    /// `reality` | `tls` | `none`.
    pub security: String,
    pub sni: Option<String>,
    pub fingerprint: Option<String>,
    /// Reality public key (`pbk` query param).
    pub public_key: Option<String>,
    pub short_id: Option<String>,
    /// `xtls-rprx-vision` etc.
    pub flow: Option<String>,
    pub path: Option<String>,
    /// xhttp mode (`packet-up`, `stream-up`, `auto`).
    pub mode: Option<String>,
    pub packet_encoding: Option<String>,
    /// All remaining query params, in case the UI wants to render them raw.
    pub raw_params: HashMap<String, String>,
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
    // `Url::username()` percent-decodes by spec but VLESS UUIDs are 8-4-4-4-12,
    // never percent-encoded. Still call decode_lossy for safety.
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
///
/// Per-line errors are silently skipped — clients often mix in comments and
/// junk lines, and one broken entry should not kill the whole list.
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

/// Try standard base64, then URL-safe, then fall back to the raw body.
fn decode_body(body: &str) -> String {
    let compact: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.contains("vless://") {
        return body.to_string();
    }
    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(compact.as_bytes()) {
        if let Ok(s) = String::from_utf8(bytes) {
            return s;
        }
    }
    if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE.decode(compact.as_bytes()) {
        if let Ok(s) = String::from_utf8(bytes) {
            return s;
        }
    }
    if let Ok(bytes) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(compact.as_bytes()) {
        if let Ok(s) = String::from_utf8(bytes) {
            return s;
        }
    }
    body.to_string()
}

fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .into_owned()
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
        assert_eq!(s.fingerprint.as_deref(), Some("chrome"));
        assert_eq!(s.public_key.as_deref(), Some("ABC"));
        assert_eq!(s.short_id.as_deref(), Some("DEAD"));
        assert_eq!(s.path.as_deref(), Some("/fi-exp-xh-1673aadd"));
        assert_eq!(s.mode.as_deref(), Some("packet-up"));
    }

    #[test]
    fn rejects_non_vless() {
        let r = parse_vless("vmess://AAAA@1.2.3.4:443");
        assert!(matches!(r, Err(ParseError::UnsupportedScheme(_))));
    }

    #[test]
    fn parses_plaintext_subscription() {
        let body = r#"
            # comment
            vless://uuid-a@host-a:443?type=tcp&security=reality#A
            vless://uuid-b@host-b:443?type=xhttp&security=reality#B
            garbage line
        "#;
        let v = parse_subscription(body);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].label, "A");
        assert_eq!(v[1].label, "B");
    }

    #[test]
    fn parses_base64_subscription() {
        let plain = "vless://uuid-a@host-a:443?type=tcp&security=reality#A\nvless://uuid-b@host-b:443?type=xhttp&security=reality#B";
        let b64 = base64::engine::general_purpose::STANDARD.encode(plain.as_bytes());
        let v = parse_subscription(&b64);
        assert_eq!(v.len(), 2);
    }
}

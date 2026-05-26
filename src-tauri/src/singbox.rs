//! Generate a sing-box client config from a parsed server + split-tunnel rules.
//!
//! Targets the sing-box 1.12+ schema. The produced config is validated against
//! the installed core (`sing-box check`) before it is used to connect.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::subscription::VlessServer;

/// Split-tunnel selection passed from the UI (only enabled entries).
///
/// One `mode` applies to BOTH the apps and sites lists. selective = whitelist
/// (only listed entries get the proxy outbound; default direct). general =
/// blacklist (all traffic uses the proxy outbound; listed entries are
/// exceptions that stay direct).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SplitInput {
    /// "selective" | "general". Empty string is treated as "general" so an
    /// uninitialised input doesn't accidentally cut the user's network.
    #[serde(default)]
    pub mode: String,
    /// Process / binary names of enabled apps.
    #[serde(default)]
    pub apps: Vec<String>,
    /// Enabled site patterns (e.g. "example.com" or "*.example.com").
    #[serde(default)]
    pub sites: Vec<String>,
}

fn is_selective(mode: &str) -> bool {
    mode == "selective"
}

/// Build the proxy outbound for the selected server.
fn build_outbound(s: &VlessServer) -> Value {
    let mut o = json!({
        "type": s.protocol,
        "tag": "proxy",
        "server": s.host,
        "server_port": s.port,
    });
    let map = o.as_object_mut().unwrap();

    match s.protocol.as_str() {
        "vless" => {
            map.insert("uuid".into(), json!(s.uuid));
            if let Some(flow) = s.flow.as_deref().filter(|f| !f.is_empty()) {
                map.insert("flow".into(), json!(flow));
            }
            map.insert("packet_encoding".into(), json!("xudp"));
        }
        "vmess" => {
            map.insert("uuid".into(), json!(s.uuid));
            map.insert("security".into(), json!("auto"));
            map.insert("alter_id".into(), json!(0));
        }
        "trojan" => {
            map.insert("password".into(), json!(s.password.clone().unwrap_or_default()));
        }
        "shadowsocks" => {
            map.insert("method".into(), json!(s.method.clone().unwrap_or_default()));
            map.insert("password".into(), json!(s.password.clone().unwrap_or_default()));
        }
        _ => {}
    }

    if let Some(tls) = build_tls(s) {
        map.insert("tls".into(), tls);
    }
    if let Some(tr) = build_transport(s) {
        map.insert("transport".into(), tr);
    }
    o
}

/// TLS / Reality block, or None for plaintext (shadowsocks, or vless `none`).
fn build_tls(s: &VlessServer) -> Option<Value> {
    let server_name = s.sni.clone().filter(|x| !x.is_empty()).unwrap_or_else(|| s.host.clone());
    let fp = s.fingerprint.clone().filter(|x| !x.is_empty()).unwrap_or_else(|| "chrome".to_string());

    match s.protocol.as_str() {
        "shadowsocks" => None,
        "trojan" | "vmess" => {
            // Trojan is always TLS; VMess only when its security says so.
            if s.protocol == "vmess" && s.security != "tls" {
                return None;
            }
            Some(json!({
                "enabled": true,
                "server_name": server_name,
                "utls": { "enabled": true, "fingerprint": fp },
            }))
        }
        _ => {
            // vless
            if s.security == "reality" {
                Some(json!({
                    "enabled": true,
                    "server_name": server_name,
                    "utls": { "enabled": true, "fingerprint": fp },
                    "reality": {
                        "enabled": true,
                        "public_key": s.public_key.clone().unwrap_or_default(),
                        "short_id": s.short_id.clone().unwrap_or_default(),
                    },
                }))
            } else if s.security == "tls" {
                Some(json!({
                    "enabled": true,
                    "server_name": server_name,
                    "utls": { "enabled": true, "fingerprint": fp },
                }))
            } else {
                None
            }
        }
    }
}

/// v2ray transport block for ws/grpc/http; None for plain tcp (incl. xhttp,
/// which sing-box doesn't implement — treated as tcp here).
fn build_transport(s: &VlessServer) -> Option<Value> {
    match s.transport.as_str() {
        "ws" => {
            let path = s.path.clone().filter(|p| !p.is_empty()).unwrap_or_else(|| "/".to_string());
            let mut t = json!({ "type": "ws", "path": path });
            if let Some(host) = s.raw_params.get("host").filter(|h| !h.is_empty()) {
                t.as_object_mut().unwrap().insert("headers".into(), json!({ "Host": host }));
            }
            Some(t)
        }
        "grpc" => {
            let service = s
                .raw_params
                .get("serviceName")
                .cloned()
                .unwrap_or_default();
            Some(json!({ "type": "grpc", "service_name": service }))
        }
        "http" => {
            let path = s.path.clone().filter(|p| !p.is_empty()).unwrap_or_else(|| "/".to_string());
            Some(json!({ "type": "http", "path": path }))
        }
        // "tcp" and "xhttp" → no v2ray transport block.
        _ => None,
    }
}

/// Route rules derived from the split-tunnel selection.
///
/// The mode controls TWO things at once: where listed entries go, and where
/// everything else goes (the route `final`):
///   - selective: listed -> proxy, default direct (whitelist).
///   - general:   listed -> direct, default proxy (blacklist).
///
/// Apps and sites both follow the same mode (the UI exposes one toggle), so
/// "selective apps + general sites" can't accidentally degrade selective
/// behaviour by silently flipping the default — the old per-list modes had
/// that bug.
fn build_route_rules(split: &SplitInput) -> (Vec<Value>, &'static str) {
    let mut rules = Vec::new();

    let selective = is_selective(&split.mode);
    let listed_outbound = if selective { "proxy" } else { "direct" };
    let default = if selective { "direct" } else { "proxy" };

    for app in &split.apps {
        if !app.is_empty() {
            rules.push(json!({ "process_name": [app], "outbound": listed_outbound }));
        }
    }

    for site in &split.sites {
        let site = site.trim();
        if site.is_empty() {
            continue;
        }
        if let Some(suffix) = site.strip_prefix("*.") {
            rules.push(json!({ "domain_suffix": [suffix], "outbound": listed_outbound }));
        } else {
            rules.push(json!({ "domain": [site], "outbound": listed_outbound }));
        }
    }

    (rules, default)
}

/// Local mixed (SOCKS5 + HTTP) inbound port used by "proxy" mode.
pub const PROXY_PORT: u16 = 2080;

/// Whole-system TUN inbound (canonical sing-box 1.12+ client setup).
fn tun_inbound() -> Value {
    json!({
        "type": "tun",
        "tag": "tun-in",
        "interface_name": "aegis0",
        "address": ["172.19.0.1/30"],
        "mtu": 1500,
        "auto_route": true,
        "strict_route": true,
        // Linux nftables fast-path; required for reliable capture with auto_route.
        "auto_redirect": true,
        // system TCP + gvisor UDP — the recommended, most reliable stack.
        "stack": "mixed"
    })
}

/// Local SOCKS5/HTTP inbound for "proxy" mode — no TUN, no root needed; apps
/// point at 127.0.0.1:PROXY_PORT.
fn proxy_inbound() -> Value {
    json!({
        "type": "mixed",
        "tag": "mixed-in",
        "listen": "127.0.0.1",
        "listen_port": PROXY_PORT
    })
}

/// Assemble the full sing-box config for the given mode ("tun" | "proxy").
/// The client tunnels everything (subject to split rules); it deliberately
/// does no geo-based bypass — geo routing is the server's concern.
pub fn build_config(server: &VlessServer, split: &SplitInput, mode: &str, allow_lan: bool) -> Value {
    let proxy_mode = mode == "proxy";
    let (split_rules, final_out) = build_route_rules(split);

    // Canonical route rule order: sniff → DNS hijack (TUN only) → optionally
    // keep private/LAN traffic direct (per the "Allow LAN" toggle) → split.
    let mut rules = vec![json!({ "action": "sniff" })];
    if !proxy_mode {
        rules.push(json!({ "protocol": "dns", "action": "hijack-dns" }));
    }
    if allow_lan {
        rules.push(json!({ "ip_is_private": true, "outbound": "direct" }));
    }
    rules.extend(split_rules);

    json!({
        "log": { "level": "warn" },
        "dns": {
            "servers": [
                { "type": "https", "tag": "remote", "server": "1.1.1.1", "detour": "proxy" },
                { "type": "udp", "tag": "local", "server": "1.1.1.1" }
            ],
            "final": "remote",
            "strategy": "prefer_ipv4"
        },
        "inbounds": [if proxy_mode { proxy_inbound() } else { tun_inbound() }],
        "outbounds": [
            build_outbound(server),
            { "type": "direct", "tag": "direct" }
        ],
        "route": {
            "rules": rules,
            "final": final_out,
            "auto_detect_interface": true,
            "default_domain_resolver": { "server": "local" }
        }
    })
}

// --- Tauri command ----------------------------------------------------------

/// Build and return the sing-box config JSON (pretty-printed) for a server +
/// split selection. Used both for inspection and (later) to connect.
#[tauri::command]
pub fn generate_singbox_config(
    server: VlessServer,
    split: SplitInput,
    mode: String,
    allow_lan: bool,
) -> Result<String, String> {
    let cfg = build_config(&server, &split, &mode, allow_lan);
    serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subscription::parse_proxy_uri;
    use base64::Engine;

    fn split() -> SplitInput {
        SplitInput::default()
    }

    #[test]
    fn vless_reality_outbound() {
        let s = parse_proxy_uri(
            "vless://uuid-1@1.2.3.4:443?type=tcp&security=reality&flow=xtls-rprx-vision&sni=icloud.com&pbk=KEY&sid=ab&fp=chrome#X",
        )
        .unwrap();
        let cfg = build_config(&s, &split(), "tun", true);
        let ob = &cfg["outbounds"][0];
        assert_eq!(ob["type"], "vless");
        assert_eq!(ob["uuid"], "uuid-1");
        assert_eq!(ob["flow"], "xtls-rprx-vision");
        assert_eq!(ob["tls"]["reality"]["enabled"], true);
        assert_eq!(ob["tls"]["reality"]["public_key"], "KEY");
        assert_eq!(ob["tls"]["server_name"], "icloud.com");
    }

    #[test]
    fn shadowsocks_outbound() {
        let creds = base64::engine::general_purpose::STANDARD.encode("aes-256-gcm:pw");
        let s = parse_proxy_uri(&format!("ss://{creds}@1.2.3.4:8388#X")).unwrap();
        let cfg = build_config(&s, &split(), "tun", true);
        let ob = &cfg["outbounds"][0];
        assert_eq!(ob["type"], "shadowsocks");
        assert_eq!(ob["method"], "aes-256-gcm");
        assert_eq!(ob["password"], "pw");
        assert!(ob.get("tls").is_none());
    }

    #[test]
    fn trojan_outbound_has_tls() {
        let s = parse_proxy_uri("trojan://pw@h.example.com:443?sni=h.example.com#X").unwrap();
        let cfg = build_config(&s, &split(), "tun", true);
        let ob = &cfg["outbounds"][0];
        assert_eq!(ob["type"], "trojan");
        assert_eq!(ob["password"], "pw");
        assert_eq!(ob["tls"]["enabled"], true);
    }

    #[test]
    fn selective_apps_route_to_proxy() {
        let s = parse_proxy_uri("vless://u@1.2.3.4:443?security=reality&pbk=K#X").unwrap();
        let sp = SplitInput {
            mode: "selective".into(),
            apps: vec!["firefox".into()],
            ..Default::default()
        };
        let cfg = build_config(&s, &sp, "tun", true);
        assert_eq!(cfg["route"]["final"], "direct");
        let rules = cfg["route"]["rules"].as_array().unwrap();
        let app_rule = rules.iter().find(|r| r.get("process_name").is_some()).unwrap();
        assert_eq!(app_rule["process_name"][0], "firefox");
        assert_eq!(app_rule["outbound"], "proxy");
    }

    #[test]
    fn selective_with_empty_lists_routes_everything_direct() {
        // Regression: previously "apps selective + sites general" (the default
        // for sites_mode after toggling apps to selective) defaulted to proxy
        // and silently made selective meaningless.
        let s = parse_proxy_uri("vless://u@1.2.3.4:443?security=reality&pbk=K#X").unwrap();
        let sp = SplitInput { mode: "selective".into(), ..Default::default() };
        let cfg = build_config(&s, &sp, "tun", true);
        assert_eq!(cfg["route"]["final"], "direct");
    }

    #[test]
    fn general_sites_route_to_direct() {
        let s = parse_proxy_uri("vless://u@1.2.3.4:443?security=reality&pbk=K#X").unwrap();
        let sp = SplitInput {
            mode: "general".into(),
            sites: vec!["*.ru".into()],
            ..Default::default()
        };
        let cfg = build_config(&s, &sp, "tun", true);
        assert_eq!(cfg["route"]["final"], "proxy");
        let rules = cfg["route"]["rules"].as_array().unwrap();
        let site_rule = rules.iter().find(|r| r.get("domain_suffix").is_some()).unwrap();
        assert_eq!(site_rule["domain_suffix"][0], "ru");
        assert_eq!(site_rule["outbound"], "direct");
    }
}

mod apps;
mod core;
mod singbox;
mod storage;
mod subscription;
mod vpn;

use std::time::{Duration, Instant};

use subscription::{
    decode_maybe_b64, extract_description, is_supported_uri, parse_headers, parse_proxy_uri,
    parse_subscription, ImportResult, VlessServer,
};

#[tauri::command]
fn parse_vless_uri(uri: String) -> Result<VlessServer, String> {
    parse_proxy_uri(&uri).map_err(|e| e.to_string())
}

#[tauri::command]
fn parse_subscription_body(body: String) -> Vec<VlessServer> {
    parse_subscription(&body)
}

/// Fetch and parse a subscription. Returns servers + server-side metadata
/// (title, update interval, traffic counters, expiry, support URL).
///
/// If `url` is a raw `vless://` link, returns a single-server result with
/// an empty meta block.
#[tauri::command]
async fn fetch_subscription(url: String) -> Result<ImportResult, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("empty URL".to_string());
    }
    if is_supported_uri(trimmed) {
        return parse_proxy_uri(trimmed)
            .map(|s| ImportResult {
                meta: Default::default(),
                servers: vec![s],
                description: None,
            })
            .map_err(|e| e.to_string());
    }

    let client = reqwest::Client::builder()
        .user_agent("AegisVPN/0.1 (sub-importer)")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let resp = client
        .get(trimmed)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let headers = resp.headers().clone();
    let meta = parse_headers(|name| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    });

    let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;
    let servers = parse_subscription(&body);
    // Prefer an in-body `# …` comment; otherwise fall back to the panel's
    // `announce` header (often base64-encoded), used by Marzban/Remnawave-style
    // panels to carry a broadcast message.
    let description = extract_description(&body).or_else(|| {
        headers
            .get("announce")
            .and_then(|v| v.to_str().ok())
            .map(decode_maybe_b64)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    });
    Ok(ImportResult { meta, servers, description })
}

/// TCP RTT to host:port in milliseconds. We open a connection and measure the
/// time until the SYN/ACK completes — effectively a layer-4 "ping" that works
/// against Reality endpoints without raw sockets / root.
///
/// Timeout is 4 seconds; returns Err on timeout or connection failure.
#[tauri::command]
async fn ping_tcp(host: String, port: u16) -> Result<u32, String> {
    let addr = format!("{host}:{port}");
    let start = Instant::now();
    match tokio::time::timeout(
        Duration::from_millis(4000),
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    {
        Ok(Ok(_stream)) => Ok(start.elapsed().as_millis().min(u32::MAX as u128) as u32),
        Ok(Err(e)) => Err(format!("connect: {e}")),
        Err(_) => Err("timeout".to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            parse_vless_uri,
            parse_subscription_body,
            fetch_subscription,
            ping_tcp,
            apps::list_installed_apps,
            apps::pick_file,
            apps::app_from_file,
            core::core_info,
            core::core_install,
            core::list_core_releases,
            singbox::generate_singbox_config,
            vpn::vpn_connect,
            vpn::vpn_disconnect,
            vpn::vpn_status,
            vpn::helper_installed,
            vpn::install_helper,
            storage::read_legacy_storage
        ])
        .on_window_event(|_window, event| {
            // Closing the window tears the VPN down too — kill the local proxy
            // (if any) and tell the helper to disconnect sing-box. Otherwise
            // the tunnel keeps running after the app is gone.
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let _ = vpn::vpn_disconnect();
            }
        })
        .setup(|_app| {
            // Devtools stay available on demand (right-click → Inspect, or the
            // shortcut) in debug builds; we just don't pop them open on launch.
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod apps;
mod core;
mod singbox;
mod storage;
mod subscription;
mod vpn;

use std::time::Duration;

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

// (Ping/latency probes are intentionally absent — pending a design pass.
// vpn::vpn_icmp_ping and the helper's PingHost request are kept so we can
// wire up either TCP, ICMP, TLS or a combination once the approach is set.)

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            parse_vless_uri,
            parse_subscription_body,
            fetch_subscription,
            apps::list_installed_apps,
            apps::pick_file,
            apps::app_from_file,
            core::core_info,
            core::core_install,
            core::core_activate,
            core::core_uninstall,
            core::list_core_releases,
            singbox::generate_singbox_config,
            vpn::vpn_connect,
            vpn::vpn_disconnect,
            vpn::vpn_status,
            vpn::helper_installed,
            vpn::install_helper,
            vpn::tcp_ping_host,
            storage::read_legacy_storage
        ])
        .on_window_event(|window, event| {
            // Closing the window must tear the VPN down too — otherwise the
            // helper's sing-box keeps running long after the GUI is gone, and
            // the user is stuck on the tunnel with no UI to disconnect.
            //
            // Tauri exits the process as soon as this handler returns, racing
            // the Unix-socket round-trip to the helper. We block the default
            // close, run the disconnect on a worker thread (sockets can
            // block), then ask the app to exit via AppHandle::exit which is
            // thread-safe and routes back through the event loop. Calling
            // `window.destroy()` from a worker thread is NOT safe in Tauri 2
            // — UI ops must happen on the main thread.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                use tauri::Manager;
                api.prevent_close();
                let app = window.app_handle().clone();
                // Run on Tauri's tokio pool — vpn_disconnect is async now
                // and we need a runtime context to await it.
                tauri::async_runtime::spawn(async move {
                    let _ = vpn::vpn_disconnect().await;
                    // Beat so the tunnel is fully torn down before the next
                    // launch, not still mid-teardown.
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    app.exit(0);
                });
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

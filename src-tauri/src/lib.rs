mod subscription;

use std::time::{Duration, Instant};

use subscription::{parse_headers, parse_subscription, parse_vless, ImportResult, VlessServer};

#[tauri::command]
fn parse_vless_uri(uri: String) -> Result<VlessServer, String> {
    parse_vless(&uri).map_err(|e| e.to_string())
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
    if trimmed.starts_with("vless://") {
        return parse_vless(trimmed)
            .map(|s| ImportResult { meta: Default::default(), servers: vec![s] })
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
    Ok(ImportResult { meta, servers })
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
            ping_tcp
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                use tauri::Manager;
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

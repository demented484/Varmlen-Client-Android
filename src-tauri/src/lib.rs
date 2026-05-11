mod subscription;

use subscription::{parse_subscription, parse_vless, VlessServer};

/// Parse a single `vless://` URI from the UI's add-link form.
#[tauri::command]
fn parse_vless_uri(uri: String) -> Result<VlessServer, String> {
    parse_vless(&uri).map_err(|e| e.to_string())
}

/// Parse subscription body that the UI already fetched.
#[tauri::command]
fn parse_subscription_body(body: String) -> Vec<VlessServer> {
    parse_subscription(&body)
}

/// Fetch a subscription URL and return parsed servers.
///
/// Returns a list (possibly empty) on success; the caller decides how to
/// present "no servers found".
#[tauri::command]
async fn fetch_subscription(url: String) -> Result<Vec<VlessServer>, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("empty URL".to_string());
    }
    if trimmed.starts_with("vless://") {
        return parse_vless(trimmed)
            .map(|s| vec![s])
            .map_err(|e| e.to_string());
    }

    let client = reqwest::Client::builder()
        .user_agent("AegisVPN/0.1 (sub-importer)")
        .timeout(std::time::Duration::from_secs(15))
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

    let body = resp
        .text()
        .await
        .map_err(|e| format!("read body: {e}"))?;

    Ok(parse_subscription(&body))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            parse_vless_uri,
            parse_subscription_body,
            fetch_subscription
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

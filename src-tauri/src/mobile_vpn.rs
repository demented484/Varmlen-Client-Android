//! Android VPN bridge. Registers the Kotlin `VpnPlugin` and forwards
//! connect/disconnect/status to it; the plugin drives the system VpnService +
//! tun2socks + the bundled xray. The xray config is the same `Tun2socks`
//! variant the desktop generates (xray as a local SOCKS proxy).

use serde::Serialize;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{AppHandle, Manager, Runtime};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectArgs {
    config: String,
    socks_port: u16,
    dns: String,
    apps: Vec<String>,
    apps_allow: bool,
    log_level: String,
}

/// Managed handle to the Android plugin.
pub struct Vpn<R: Runtime>(PluginHandle<R>);

/// Tauri plugin that registers the Android `VpnPlugin`.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("varmlenvpn")
        .setup(|app, api| {
            let handle = api.register_android_plugin("app.varmlen.client", "VpnPlugin")?;
            app.manage(Vpn(handle));
            Ok(())
        })
        .build()
}

/// Start the VPN: hand the generated xray config + per-app split to the service.
pub fn connect<R: Runtime>(
    app: &AppHandle<R>,
    config: String,
    socks_port: u16,
    apps: Vec<String>,
    apps_allow: bool,
    log_level: String,
) -> Result<(), String> {
    let vpn = app.state::<Vpn<R>>();
    vpn.0
        .run_mobile_plugin::<serde_json::Value>(
            "connect",
            ConnectArgs {
                config,
                socks_port,
                dns: "1.1.1.1".to_string(),
                apps,
                apps_allow,
                log_level,
            },
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Read the on-device VPN log (the VpnService writes it to filesDir).
pub fn read_log<R: Runtime>(app: &AppHandle<R>) -> Result<String, String> {
    let vpn = app.state::<Vpn<R>>();
    vpn.0
        .run_mobile_plugin::<serde_json::Value>("readLog", ())
        .map(|v| {
            v.get("log")
                .and_then(|l| l.as_str())
                .unwrap_or("")
                .to_string()
        })
        .map_err(|e| e.to_string())
}

pub fn clear_log<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let vpn = app.state::<Vpn<R>>();
    vpn.0
        .run_mobile_plugin::<serde_json::Value>("clearLog", ())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
struct AppsResp {
    apps: Vec<crate::apps::InstalledApp>,
}

/// The launchable packages on the device (for the split-tunnel app picker).
pub fn list_apps<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<crate::apps::InstalledApp>, String> {
    let vpn = app.state::<Vpn<R>>();
    vpn.0
        .run_mobile_plugin::<AppsResp>("listApps", ())
        .map(|r| r.apps)
        .map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
struct XrayPaths {
    bin: String,
    dir: String,
}

/// (xray binary path in nativeLibraryDir, a writable config dir) — so the Rust
/// proxy-ping can run xray on Android too.
pub fn xray_paths<R: Runtime>(app: &AppHandle<R>) -> Result<(String, String), String> {
    let vpn = app.state::<Vpn<R>>();
    vpn.0
        .run_mobile_plugin::<XrayPaths>("xrayPaths", ())
        .map(|p| (p.bin, p.dir))
        .map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
struct ClipResp {
    text: String,
}

/// Read the system clipboard (WebView blocks navigator.clipboard on Android).
pub fn read_clipboard<R: Runtime>(app: &AppHandle<R>) -> Result<String, String> {
    let vpn = app.state::<Vpn<R>>();
    vpn.0
        .run_mobile_plugin::<ClipResp>("readClipboard", ())
        .map(|r| r.text)
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct BarStyleArgs {
    light: bool,
}

/// Set the system-bar icon colour to match the app theme.
pub fn set_bar_style<R: Runtime>(app: &AppHandle<R>, light: bool) -> Result<(), String> {
    let vpn = app.state::<Vpn<R>>();
    vpn.0
        .run_mobile_plugin::<serde_json::Value>("setBarStyle", BarStyleArgs { light })
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
struct NotifResp {
    enabled: bool,
}

/// Whether the app may post notifications (the VPN status notice).
pub fn notifications_enabled<R: Runtime>(app: &AppHandle<R>) -> Result<bool, String> {
    let vpn = app.state::<Vpn<R>>();
    vpn.0
        .run_mobile_plugin::<NotifResp>("notificationsEnabled", ())
        .map(|r| r.enabled)
        .map_err(|e| e.to_string())
}

/// Open the system notification settings for this app (to grant after a decline).
pub fn open_notification_settings<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let vpn = app.state::<Vpn<R>>();
    vpn.0
        .run_mobile_plugin::<serde_json::Value>("openNotificationSettings", ())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn disconnect<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let vpn = app.state::<Vpn<R>>();
    vpn.0
        .run_mobile_plugin::<serde_json::Value>("disconnect", ())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn is_running<R: Runtime>(app: &AppHandle<R>) -> bool {
    let vpn = app.state::<Vpn<R>>();
    vpn.0
        .run_mobile_plugin::<serde_json::Value>("status", ())
        .ok()
        .and_then(|v| v.get("running").and_then(|r| r.as_bool()))
        .unwrap_or(false)
}

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
            },
        )
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

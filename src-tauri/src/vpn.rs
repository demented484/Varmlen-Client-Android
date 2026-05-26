//! Client side of connect/disconnect.
//!
//! - "tun" mode: full-system TUN, run by the root `aegisvpn-helper` (needs
//!   CAP_NET_ADMIN). We hand it the generated config over a Unix socket.
//! - "proxy" mode: a local SOCKS5/HTTP inbound, run directly by the client as
//!   the current user (no root, no helper). Apps point at 127.0.0.1:PROXY_PORT.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::singbox::{build_config, SplitInput};
use crate::subscription::VlessServer;

const SOCKET: &str = "/run/aegisvpn/helper.sock";

#[derive(Serialize, Deserialize)]
pub struct HelperResponse {
    pub ok: bool,
    pub state: String,
    pub pid: Option<u32>,
    pub error: Option<String>,
}

impl HelperResponse {
    fn connected(pid: u32) -> Self {
        HelperResponse { ok: true, state: "connected".into(), pid: Some(pid), error: None }
    }
    fn disconnected() -> Self {
        HelperResponse { ok: true, state: "disconnected".into(), pid: None, error: None }
    }
}

// --- helper (TUN, root) socket -------------------------------------------------

fn send(req: serde_json::Value) -> Result<HelperResponse, String> {
    let stream = UnixStream::connect(SOCKET).map_err(|_| {
        "helper not reachable — set up the helper in Settings (TUN mode needs it)".to_string()
    })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(25)))
        .map_err(|e| e.to_string())?;

    let mut line = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    line.push('\n');
    {
        let mut w = &stream;
        w.write_all(line.as_bytes()).map_err(|e| format!("helper write: {e}"))?;
    }

    let mut reader = BufReader::new(&stream);
    let mut resp = String::new();
    reader.read_line(&mut resp).map_err(|e| format!("helper read: {e}"))?;
    serde_json::from_str(resp.trim()).map_err(|e| format!("helper response: {e}"))
}

// --- local proxy (SOCKS/HTTP, no root) ----------------------------------------

/// The locally-spawned sing-box (proxy mode), owned by this process.
fn local_child() -> &'static Mutex<Option<Child>> {
    static C: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

fn stop_local() {
    if let Some(mut c) = local_child().lock().unwrap().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

fn local_alive() -> Option<u32> {
    let mut guard = local_child().lock().unwrap();
    match guard.as_mut() {
        Some(c) => match c.try_wait() {
            Ok(None) => Some(c.id()),
            _ => {
                *guard = None;
                None
            }
        },
        None => None,
    }
}

fn last_error_line(stderr: &str) -> String {
    let strip = |s: &str| s.replace(|c: char| c == '\u{1b}', "");
    stderr
        .lines()
        .map(|l| strip(l).trim().to_string())
        .filter(|l| !l.is_empty())
        .find(|l| l.contains("FATAL") || l.contains("ERROR"))
        .or_else(|| {
            stderr
                .lines()
                .map(|l| strip(l).trim().to_string())
                .filter(|l| !l.is_empty())
                .last()
        })
        .unwrap_or_else(|| "no output".to_string())
}

fn proxy_connect(app: &tauri::AppHandle, config: &str) -> Result<HelperResponse, String> {
    stop_local();
    let core = crate::core::binary_path(app)?;
    if !core.exists() {
        return Err("install the sing-box core first (Settings → VPN core)".into());
    }
    let cfg_path = std::env::temp_dir().join("aegisvpn-proxy.json");
    std::fs::write(&cfg_path, config).map_err(|e| format!("write config: {e}"))?;

    let mut child = Command::new(&core)
        .arg("run")
        .arg("-c")
        .arg(&cfg_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn sing-box: {e}"))?;
    let pid = child.id();

    std::thread::sleep(Duration::from_millis(800));
    if let Ok(Some(_)) = child.try_wait() {
        let mut err = String::new();
        if let Some(mut s) = child.stderr.take() {
            let _ = s.read_to_string(&mut err);
        }
        return Err(format!("sing-box exited: {}", last_error_line(&err)));
    }
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                eprintln!("sing-box: {line}");
            }
        });
    }
    *local_child().lock().unwrap() = Some(child);
    Ok(HelperResponse::connected(pid))
}

// --- commands -----------------------------------------------------------------

#[tauri::command]
pub fn vpn_connect(
    app: tauri::AppHandle,
    server: VlessServer,
    split: SplitInput,
    mode: String,
    killswitch: bool,
    allow_lan: bool,
) -> Result<HelperResponse, String> {
    let cfg = build_config(&server, &split, &mode, allow_lan);
    let cfg_str = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;

    if mode == "proxy" {
        // Tear down any TUN session first, then run a local proxy.
        let _ = send(json!({ "cmd": "disconnect" }));
        proxy_connect(&app, &cfg_str)
    } else {
        stop_local();
        send(json!({
            "cmd": "connect",
            "config": cfg_str,
            "killswitch": killswitch,
            "allow_lan": allow_lan,
            "server": server.host,
        }))
    }
}

#[tauri::command]
pub fn vpn_disconnect() -> Result<HelperResponse, String> {
    stop_local();
    let _ = send(json!({ "cmd": "disconnect" })); // ignore if helper absent
    Ok(HelperResponse::disconnected())
}

#[tauri::command]
pub fn vpn_status() -> Result<HelperResponse, String> {
    if let Some(pid) = local_alive() {
        return Ok(HelperResponse::connected(pid));
    }
    match send(json!({ "cmd": "status" })) {
        Ok(r) => Ok(r),
        Err(_) => Ok(HelperResponse::disconnected()),
    }
}

/// Whether the privileged helper is installed and its socket is reachable.
#[tauri::command]
pub fn helper_installed() -> bool {
    UnixStream::connect(SOCKET).is_ok()
}

/// Install the privileged helper via a one-time polkit (pkexec) prompt. The
/// installer copies the prebuilt helper + the downloaded sing-box core to a
/// root-owned location and enables the systemd service.
#[tauri::command]
pub fn install_helper(app: tauri::AppHandle) -> Result<(), String> {
    let core = crate::core::binary_path(&app)?;
    if !core.exists() {
        return Err("install the sing-box core first (Settings → VPN core)".into());
    }

    // Dev path; a packaged build ships the installer as a bundled resource.
    let script = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../helper/install.sh"));
    if !script.exists() {
        return Err(format!("installer not found at {}", script.display()));
    }

    let status = std::process::Command::new("pkexec")
        .arg(script)
        .arg(core.to_string_lossy().as_ref())
        .status()
        .map_err(|e| format!("pkexec: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "helper install failed or was cancelled (exit {:?})",
            status.code()
        ))
    }
}

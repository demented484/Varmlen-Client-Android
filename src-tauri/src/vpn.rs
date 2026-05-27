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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtt_ms: Option<u32>,
}

impl HelperResponse {
    fn connected(pid: u32) -> Self {
        HelperResponse { ok: true, state: "connected".into(), pid: Some(pid), error: None, rtt_ms: None }
    }
    fn disconnected() -> Self {
        HelperResponse { ok: true, state: "disconnected".into(), pid: None, error: None, rtt_ms: None }
    }
}

// --- helper (TUN, root) socket -------------------------------------------------

fn send_with_timeout(
    req: serde_json::Value,
    read_timeout: Duration,
) -> Result<HelperResponse, String> {
    let stream = UnixStream::connect(SOCKET).map_err(|_| {
        "helper not reachable — set up the helper in Settings (TUN mode needs it)".to_string()
    })?;
    stream.set_read_timeout(Some(read_timeout)).map_err(|e| e.to_string())?;
    stream.set_write_timeout(Some(Duration::from_secs(2))).map_err(|e| e.to_string())?;

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

fn send(req: serde_json::Value) -> Result<HelperResponse, String> {
    send_with_timeout(req, Duration::from_secs(25))
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

/// All Tauri commands here are `async` on purpose. Sync `#[tauri::command]`
/// runs on the main event-loop thread, which blocks WebKit rendering for the
/// duration of the call — every `std::thread::sleep` in the connect path
/// (helper validation, sing-box startup wait, socket round-trips) would
/// freeze the UI, so users never saw the "Connecting" state. Async commands
/// run on Tauri's tokio pool and leave the main thread free to repaint.
/// `spawn_blocking` wraps the helper-socket / process I/O so it doesn't tie
/// up async workers either.
#[tauri::command]
pub async fn vpn_connect(
    app: tauri::AppHandle,
    server: VlessServer,
    split: SplitInput,
    mode: String,
    killswitch: bool,
    allow_lan: bool,
) -> Result<HelperResponse, String> {
    let cfg = build_config(&server, &split, &mode, allow_lan);
    let cfg_str = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;
    let host = server.host.clone();

    if mode == "proxy" {
        // Tear down any TUN session first, then run a local proxy.
        let _ = tokio::task::spawn_blocking(|| send(json!({ "cmd": "disconnect" }))).await;
        tokio::task::spawn_blocking(move || proxy_connect(&app, &cfg_str))
            .await
            .map_err(|e| format!("join: {e}"))?
    } else {
        tokio::task::spawn_blocking(move || {
            stop_local();
            send(json!({
                "cmd": "connect",
                "config": cfg_str,
                "killswitch": killswitch,
                "allow_lan": allow_lan,
                "server": host,
            }))
        })
        .await
        .map_err(|e| format!("join: {e}"))?
    }
}

#[tauri::command]
pub async fn vpn_disconnect() -> Result<HelperResponse, String> {
    let _ = tokio::task::spawn_blocking(|| {
        stop_local();
        // Tight timeout: this is called on window close — we don't want the
        // GUI to hang for 25s if the helper socket is wedged. Helper teardown
        // (terminate sing-box + kill_stray_core + remove_killswitch) is
        // bounded to ~6s in the worst case; 8s leaves headroom.
        let _ = send_with_timeout(json!({ "cmd": "disconnect" }), Duration::from_secs(8));
    })
    .await;
    Ok(HelperResponse::disconnected())
}

#[tauri::command]
pub async fn vpn_status() -> Result<HelperResponse, String> {
    if let Some(pid) = local_alive() {
        return Ok(HelperResponse::connected(pid));
    }
    let r = tokio::task::spawn_blocking(|| send(json!({ "cmd": "status" })))
        .await
        .map_err(|e| format!("join: {e}"))?;
    Ok(r.unwrap_or_else(|_| HelperResponse::disconnected()))
}

/// Whether the privileged helper is installed and its socket is reachable.
#[tauri::command]
pub async fn helper_installed() -> bool {
    tokio::task::spawn_blocking(|| UnixStream::connect(SOCKET).is_ok())
        .await
        .unwrap_or(false)
}

/// Synchronise the helper's CORE binary to a user-side path (called by the
/// core manager when the user activates / re-installs a version). The helper
/// reads + copies it to its fixed root-owned location.
pub fn helper_install_core(path: std::path::PathBuf) -> Result<(), String> {
    let resp = send(json!({ "cmd": "install_core", "path": path.to_string_lossy() }))?;
    if resp.ok {
        Ok(())
    } else {
        Err(resp.error.unwrap_or_else(|| "helper rejected core install".into()))
    }
}

/// First non-loopback non-tunnel IPv4 the kernel advertises. Used as the
/// source IP for ping probes so that a TUN-mode VPN tunnel doesn't swallow
/// them (without source-binding, every probe would just measure the active
/// server's RTT). Returns None when no physical interface has an address.
fn physical_ipv4() -> Option<std::net::Ipv4Addr> {
    unsafe {
        let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifap) != 0 {
            return None;
        }
        let mut found = None;
        let mut cur = ifap;
        while !cur.is_null() {
            let ifa = &*cur;
            if !ifa.ifa_name.is_null() && !ifa.ifa_addr.is_null() {
                let name = std::ffi::CStr::from_ptr(ifa.ifa_name).to_string_lossy();
                let virt = name.starts_with("lo")
                    || name.starts_with("tun")
                    || name.starts_with("tap")
                    || name.starts_with("wg")
                    || name.starts_with("docker")
                    || name.starts_with("br-")
                    || name.starts_with("veth")
                    || name.starts_with("vmnet");
                let sa = &*ifa.ifa_addr;
                if !virt && sa.sa_family as i32 == libc::AF_INET {
                    let sin = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                    let ip = std::net::Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
                    if !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified() {
                        found = Some(ip);
                        break;
                    }
                }
            }
            cur = ifa.ifa_next;
        }
        libc::freeifaddrs(ifap);
        found
    }
}

/// TCP-connect RTT to host:port in ms. No root needed — measures the TCP
/// 3-way handshake the same way Happ does for its location pings. The probe
/// is source-bound to the physical interface so the result reflects the
/// user's real network path, not the active VPN tunnel.
#[tauri::command]
pub async fn tcp_ping_host(host: String, port: u16, timeout_ms: Option<u32>) -> Result<u32, String> {
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(2500) as u64);
    tokio::task::spawn_blocking(move || {
        use socket2::{Domain, Protocol, SockAddr, Socket, Type};
        use std::net::{SocketAddr, ToSocketAddrs};

        let dst: SocketAddr = (host.as_str(), port)
            .to_socket_addrs()
            .map_err(|e| format!("resolve: {e}"))?
            .find(|a| a.is_ipv4())
            .ok_or_else(|| "no ipv4 addr".to_string())?;

        let sock = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
            .map_err(|e| format!("socket: {e}"))?;
        // Best-effort source-bind; absent a physical iface (eg. user on TUN-only
        // setup) we let the kernel pick — the ping will go via the tunnel but
        // the call still succeeds.
        if let Some(ip) = physical_ipv4() {
            let _ = sock.bind(&SockAddr::from(SocketAddr::from((ip, 0))));
        }

        let started = std::time::Instant::now();
        sock.connect_timeout(&SockAddr::from(dst), timeout)
            .map_err(|e| format!("connect: {e}"))?;
        Ok(started.elapsed().as_millis().min(u32::MAX as u128) as u32)
    })
    .await
    .map_err(|e| format!("join: {e}"))?
}

/// ICMP RTT to host via the privileged helper (raw ICMP needs root). Returns
/// the time in ms, or an error string when unreachable / helper is absent.
///
/// Not currently wired to the frontend — kept available for when the ping UI
/// returns. Suppress dead-code so cargo check stays clean.
#[allow(dead_code)]
pub fn vpn_icmp_ping(host: String, timeout_ms: Option<u32>) -> Result<u32, String> {
    let resp = send(json!({
        "cmd": "ping_host",
        "host": host,
        "timeout_ms": timeout_ms.unwrap_or(2000),
    }))?;
    if resp.ok {
        resp.rtt_ms.ok_or_else(|| "helper returned no rtt".into())
    } else {
        Err(resp.error.unwrap_or_else(|| "ping failed".into()))
    }
}

/// Install the privileged helper via a one-time polkit (pkexec) prompt. The
/// installer copies the prebuilt helper + the downloaded sing-box core to a
/// root-owned location and enables the systemd service.
///
/// Async + spawn_blocking: the pkexec prompt can take 30+ seconds while the
/// user types their password — blocking the main thread would freeze the UI.
#[tauri::command]
pub async fn install_helper(app: tauri::AppHandle) -> Result<(), String> {
    tokio::task::spawn_blocking(move || install_helper_blocking(app))
        .await
        .map_err(|e| format!("join: {e}"))?
}

fn install_helper_blocking(app: tauri::AppHandle) -> Result<(), String> {
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

//! Protocol-aware "real" ping.
//!
//! TCP-only ping (the cheap `connect()` RTT) tells you the server is reachable,
//! but says nothing about whether the protocol works — wrong Reality key, wrong
//! SNI, blocked transport, expired uuid all happily complete a TCP handshake.
//!
//! Here we spin up the managed sing-box as a one-shot SOCKS proxy with just
//! this server as outbound, then HTTP-GET a known 204 endpoint through it. The
//! time-to-204 is the protocol-verified RTT; any failure (handshake reject,
//! TLS error, timeout) returns an error so the UI can mark the location dead.
//!
//! Each call spawns its own sing-box process on a free port; callers should
//! limit concurrency (we pin one at a time per server, and the UI should
//! batch).
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::AppHandle;
use tokio::process::{Child, Command};

use crate::core::binary_path;
use crate::singbox::build_outbound_public;
use crate::subscription::VlessServer;

/// Endpoint we GET through the proxy. `generate_204` returns an empty 204 on
/// success — cheap, well-known, served from a CDN close to most regions.
const PROBE_URL: &str = "https://www.gstatic.com/generate_204";

/// Find a free localhost port by binding to :0 and releasing it. There's a
/// race against another process grabbing it before sing-box starts, but the
/// port is only useful for that ~one-shot, so it's acceptable in practice.
fn pick_free_port() -> Result<u16, String> {
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind: {e}"))?;
    let port = l.local_addr().map_err(|e| format!("local_addr: {e}"))?.port();
    drop(l);
    Ok(port)
}

/// Minimal sing-box config: one outbound (the server under test) and a mixed
/// inbound on a free localhost port. No TUN, no killswitch, no split — we just
/// need the proxy to attempt the protocol handshake.
fn ping_config(server: &VlessServer, port: u16) -> serde_json::Value {
    json!({
        "log": { "level": "error" },
        "inbounds": [{
            "type": "mixed",
            "tag": "in",
            "listen": "127.0.0.1",
            "listen_port": port
        }],
        "outbounds": [
            build_outbound_public(server),
            { "type": "direct", "tag": "direct" }
        ],
        "route": { "final": "proxy" }
    })
}

/// Write `cfg` to a fresh temp file the caller is responsible for cleaning up.
fn write_temp_config(cfg: &serde_json::Value) -> Result<PathBuf, String> {
    let bytes = serde_json::to_vec(cfg).map_err(|e| format!("serialize: {e}"))?;
    let pid = std::process::id();
    let nonce: u64 = rand_u64();
    let mut path = std::env::temp_dir();
    path.push(format!("aegisvpn-ping-{pid}-{nonce}.json"));
    std::fs::write(&path, &bytes).map_err(|e| format!("write temp: {e}"))?;
    Ok(path)
}

/// Cheap non-cryptographic randomness for the temp-file name. Avoids pulling
/// in `rand` just for this.
fn rand_u64() -> u64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    (nanos as u64) ^ (std::process::id() as u64).rotate_left(17)
}

/// Wait until the proxy port accepts a connection, or give up.
async fn wait_for_proxy(port: u16, total: Duration) -> Result<(), String> {
    let deadline = Instant::now() + total;
    let mut delay = Duration::from_millis(40);
    while Instant::now() < deadline {
        if tokio::time::timeout(
            Duration::from_millis(200),
            tokio::net::TcpStream::connect(("127.0.0.1", port)),
        )
        .await
        .ok()
        .and_then(|r| r.ok())
        .is_some()
        {
            return Ok(());
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_millis(300));
    }
    Err("sing-box didn't open the proxy port in time".to_string())
}

/// Kill the child sing-box and reap it. Best-effort — never propagates errors.
async fn cleanup(mut child: Child, cfg_path: PathBuf) {
    let _ = child.start_kill();
    let _ = tokio::time::timeout(Duration::from_millis(500), child.wait()).await;
    let _ = std::fs::remove_file(&cfg_path);
}

/// Real RTT through the proxy: time from when we issue the GET to when the
/// 204 lands. This includes TLS / Reality / VLESS handshake, so it's a true
/// "this protocol works" measurement.
async fn measure(port: u16, timeout: Duration) -> Result<u32, String> {
    let proxy = reqwest::Proxy::all(format!("socks5h://127.0.0.1:{port}"))
        .map_err(|e| format!("proxy: {e}"))?;
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(timeout)
        .danger_accept_invalid_certs(false)
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let start = Instant::now();
    let resp = client
        .get(PROBE_URL)
        .send()
        .await
        .map_err(|e| format!("probe failed: {e}"))?;
    let status = resp.status();
    if !(status.is_success() || status.as_u16() == 204) {
        return Err(format!("probe HTTP {status}"));
    }
    Ok(start.elapsed().as_millis().min(u32::MAX as u128) as u32)
}

/// Verify the server works with its protocol + measure RTT to a known endpoint
/// through it. Returns milliseconds, or an error string the UI surfaces.
#[tauri::command]
pub async fn ping_protocol(
    app: AppHandle,
    server: VlessServer,
    timeout_ms: Option<u64>,
) -> Result<u32, String> {
    let total = Duration::from_millis(timeout_ms.unwrap_or(6000));
    let bin = binary_path(&app)?;
    if !bin.exists() {
        return Err("sing-box core is not installed".to_string());
    }

    let port = pick_free_port()?;
    let cfg = ping_config(&server, port);
    let cfg_path = write_temp_config(&cfg)?;

    let child = Command::new(&bin)
        .arg("run")
        .arg("-c")
        .arg(&cfg_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("spawn sing-box: {e}"))?;

    // Probe budget: leave ~1s for shutdown; cap the readiness wait at 2s, give
    // the GET whatever's left (min 1.5s).
    let ready_budget = Duration::from_millis(2000).min(total / 3);
    let req_budget = total.saturating_sub(ready_budget).max(Duration::from_millis(1500));

    let result: Result<u32, String> = async {
        wait_for_proxy(port, ready_budget).await?;
        measure(port, req_budget).await
    }
    .await;

    cleanup(child, cfg_path).await;
    result
}

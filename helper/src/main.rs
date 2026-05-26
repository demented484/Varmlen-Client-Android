//! AegisVPN privileged helper.
//!
//! Runs as a root systemd service and owns the sing-box process (which needs
//! CAP_NET_ADMIN for its TUN device). The unprivileged GUI client talks to it
//! over a Unix socket using newline-delimited JSON.
//!
//! Security: only the UID in `AEGIS_ALLOW_UID` (set by the installer's unit)
//! — plus root — may issue commands, verified via SO_PEERCRED.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

const RUN_DIR: &str = "/run/aegisvpn";
const SOCKET: &str = "/run/aegisvpn/helper.sock";
const CONFIG: &str = "/run/aegisvpn/config.json";
/// The sing-box binary the helper runs. It is intentionally a fixed,
/// root-owned path installed by the helper installer — never a path supplied by
/// the (unprivileged) client, which would let a local user run an arbitrary
/// binary as root.
const CORE: &str = "/usr/local/lib/aegisvpn/sing-box";

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "lowercase")]
enum Request {
    /// Start sing-box with the given config (the core binary is fixed).
    Connect {
        config: String,
        #[serde(default)]
        killswitch: bool,
        #[serde(default)]
        allow_lan: bool,
        /// Proxy server host, to allow-list through the killswitch.
        #[serde(default)]
        server: String,
    },
    Disconnect,
    Status,
    Ping,
}

const KS_TABLE: &str = "aegis_ks";

#[derive(Serialize)]
struct Response {
    ok: bool,
    state: String,
    pid: Option<u32>,
    error: Option<String>,
}

impl Response {
    fn ok(state: &str, pid: Option<u32>) -> Self {
        Response { ok: true, state: state.into(), pid, error: None }
    }
    fn err(state: &str, msg: String) -> Self {
        Response { ok: false, state: state.into(), pid: None, error: Some(msg) }
    }
}

#[derive(Default)]
struct Daemon {
    child: Option<Child>,
}

impl Daemon {
    /// Current state, reaping the child if it has exited.
    fn state(&mut self) -> &'static str {
        match self.child.as_mut() {
            Some(c) => match c.try_wait() {
                Ok(None) => "connected",
                _ => {
                    self.child = None;
                    "disconnected"
                }
            },
            None => "disconnected",
        }
    }

    fn disconnect(&mut self) {
        if let Some(mut c) = self.child.take() {
            terminate_gracefully(&mut c);
        }
        // Explicit disconnect → drop the killswitch so the user has network.
        remove_killswitch();
    }

    fn connect(
        &mut self,
        config: &str,
        killswitch: bool,
        allow_lan: bool,
        server: &str,
    ) -> Result<u32, String> {
        // Stop the old core gracefully (so it removes its routes) but keep the
        // killswitch up across the gap.
        if let Some(mut c) = self.child.take() {
            terminate_gracefully(&mut c);
        }

        if !std::path::Path::new(CORE).exists() {
            remove_killswitch();
            return Err(format!("sing-box core not installed at {CORE}"));
        }

        // Apply (or refresh) the killswitch before launching, so there's no
        // leak window. On any failure below we tear it down to avoid locking
        // the user out of the network.
        if killswitch {
            let ips = resolve_ips(server);
            if let Err(e) = apply_killswitch(&ips, allow_lan) {
                eprintln!("killswitch: {e}");
            }
        } else {
            remove_killswitch();
        }

        // Write config, validate, launch, and confirm it stays up. Any failure
        // tears the killswitch back down so the user keeps their network.
        let started: Result<Child, String> = (|| {
            std::fs::create_dir_all(RUN_DIR).map_err(|e| format!("run dir: {e}"))?;
            std::fs::write(CONFIG, config).map_err(|e| format!("write config: {e}"))?;
            let _ = std::fs::set_permissions(CONFIG, std::fs::Permissions::from_mode(0o600));

            let check = Command::new(CORE)
                .arg("check")
                .arg("-c")
                .arg(CONFIG)
                .output()
                .map_err(|e| format!("run core: {e}"))?;
            if !check.status.success() {
                let msg = String::from_utf8_lossy(&check.stderr);
                return Err(format!("config rejected: {}", msg.trim()));
            }

            let mut child = Command::new(CORE)
                .arg("run")
                .arg("-c")
                .arg(CONFIG)
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("spawn sing-box: {e}"))?;

            // A bad config / missing privilege makes it exit within a moment.
            std::thread::sleep(Duration::from_millis(900));
            if let Ok(Some(_)) = child.try_wait() {
                let mut err = String::new();
                if let Some(mut s) = child.stderr.take() {
                    let _ = s.read_to_string(&mut err);
                }
                return Err(format!("sing-box exited: {}", last_error_line(&err)));
            }
            Ok(child)
        })();

        let mut child = match started {
            Ok(c) => c,
            Err(e) => {
                remove_killswitch(); // don't leave the user blocked on a failed connect
                return Err(e);
            }
        };
        let pid = child.id();

        // Alive: drain its stderr into the journal so the pipe never fills up.
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    eprintln!("sing-box: {line}");
                }
            });
        }

        self.child = Some(child);
        Ok(pid)
    }
}

/// Resolve a proxy server host to its IP addresses (for the killswitch
/// allow-list). Uses normal resolution before the killswitch is applied.
fn resolve_ips(host: &str) -> Vec<std::net::IpAddr> {
    use std::net::ToSocketAddrs;
    if host.is_empty() {
        return Vec::new();
    }
    (host, 443u16)
        .to_socket_addrs()
        .map(|it| it.map(|s| s.ip()).collect())
        .unwrap_or_default()
}

/// Build + apply the killswitch ruleset: drop all output except loopback, the
/// tunnel, the proxy server, DNS bootstrap, and (optionally) LAN. Atomic via a
/// single `nft -f` transaction so reconnects never open a leak window.
fn apply_killswitch(server_ips: &[std::net::IpAddr], allow_lan: bool) -> Result<(), String> {
    let mut r = String::new();
    // add+delete makes the transaction idempotent whether or not it existed.
    r.push_str(&format!("add table inet {KS_TABLE}\n"));
    r.push_str(&format!("delete table inet {KS_TABLE}\n"));
    r.push_str(&format!("table inet {KS_TABLE} {{\n"));
    r.push_str("  chain out {\n");
    r.push_str("    type filter hook output priority 0; policy drop;\n");
    r.push_str("    oifname \"lo\" accept\n");
    r.push_str("    oifname \"aegis0\" accept\n");
    r.push_str("    ct state established,related accept\n");
    // DNS bootstrap to the resolver the config uses, so sing-box can resolve
    // the server host while the tunnel is down.
    r.push_str("    ip daddr 1.1.1.1 udp dport 53 accept\n");
    r.push_str("    ip daddr 1.1.1.1 tcp dport 53 accept\n");
    for ip in server_ips {
        match ip {
            std::net::IpAddr::V4(v4) => r.push_str(&format!("    ip daddr {v4} accept\n")),
            std::net::IpAddr::V6(v6) => r.push_str(&format!("    ip6 daddr {v6} accept\n")),
        }
    }
    if allow_lan {
        r.push_str("    ip daddr { 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16 } accept\n");
        r.push_str("    ip6 daddr { fe80::/10, fc00::/7 } accept\n");
    }
    r.push_str("  }\n}\n");

    let mut child = Command::new("nft")
        .arg("-f")
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("nft spawn: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(r.as_bytes()).map_err(|e| format!("nft write: {e}"))?;
    }
    let status = child.wait().map_err(|e| format!("nft wait: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("nft apply failed".to_string())
    }
}

/// Remove the killswitch table (no-op if absent).
fn remove_killswitch() {
    let _ = Command::new("nft")
        .arg("delete")
        .arg("table")
        .arg("inet")
        .arg(KS_TABLE)
        .stderr(Stdio::null())
        .status();
}

/// Pick the most useful line out of sing-box stderr (the FATAL/ERROR, stripped
/// of ANSI colour codes), falling back to the last non-empty line.
fn last_error_line(stderr: &str) -> String {
    let clean = |s: &str| -> String {
        // Drop ANSI escape sequences (\x1b[...m).
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out.trim().to_string()
    };
    stderr
        .lines()
        .map(clean)
        .filter(|l| !l.is_empty())
        .find(|l| l.contains("FATAL") || l.contains("ERROR"))
        .or_else(|| stderr.lines().map(clean).filter(|l| !l.is_empty()).last())
        .unwrap_or_else(|| "no output".to_string())
}

/// Stop sing-box gracefully: SIGTERM so it can tear down its TUN routes and
/// nftables rules (a SIGKILL would leave the network unroutable), then wait,
/// with a SIGKILL fallback if it doesn't exit in time.
fn terminate_gracefully(child: &mut Child) {
    let pid = child.id() as libc::pid_t;
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    for _ in 0..50 {
        match child.try_wait() {
            Ok(Some(_)) => return,
            _ => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let _ = child.kill(); // SIGKILL fallback after ~5s
    let _ = child.wait();
}

fn peer_uid(stream: &UnixStream) -> Option<u32> {
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if rc == 0 {
        Some(cred.uid)
    } else {
        None
    }
}

fn handle(stream: UnixStream, daemon: Arc<Mutex<Daemon>>) {
    let mut writer = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let resp = match serde_json::from_str::<Request>(&line) {
            Ok(req) => {
                let mut d = daemon.lock().unwrap();
                match req {
                    Request::Ping => Response::ok(d.state(), None),
                    Request::Status => Response::ok(d.state(), d.child.as_ref().map(|c| c.id())),
                    Request::Disconnect => {
                        d.disconnect();
                        Response::ok("disconnected", None)
                    }
                    Request::Connect { config, killswitch, allow_lan, server } => {
                        match d.connect(&config, killswitch, allow_lan, &server) {
                            Ok(pid) => Response::ok("connected", Some(pid)),
                            Err(e) => Response::err(d.state(), e),
                        }
                    }
                }
            }
            Err(e) => Response::err("unknown", format!("bad request: {e}")),
        };
        let mut out = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".into());
        out.push('\n');
        if writer.write_all(out.as_bytes()).is_err() {
            break;
        }
    }
}

fn main() {
    let allowed_uid: Option<u32> = std::env::var("AEGIS_ALLOW_UID")
        .ok()
        .and_then(|v| v.trim().parse().ok());

    // Clear any killswitch left over from a previous run / crash so we never
    // start up blocking the user's network.
    remove_killswitch();

    let _ = std::fs::create_dir_all(RUN_DIR);
    let _ = std::fs::remove_file(SOCKET);
    let listener = UnixListener::bind(SOCKET).expect("bind helper socket");
    // World-accessible socket; access is actually gated by the SO_PEERCRED check.
    let _ = std::fs::set_permissions(SOCKET, std::fs::Permissions::from_mode(0o666));

    let daemon = Arc::new(Mutex::new(Daemon::default()));

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if let Some(allow) = allowed_uid {
            match peer_uid(&stream) {
                Some(uid) if uid == allow || uid == 0 => {}
                _ => continue, // reject unauthorized peers silently
            }
        }
        let daemon = daemon.clone();
        std::thread::spawn(move || handle(stream, daemon));
    }
}

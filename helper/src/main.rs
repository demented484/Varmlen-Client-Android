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
#[serde(tag = "cmd", rename_all = "snake_case")]
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
    /// Liveness check.
    Ping,
    /// Copy a user-supplied sing-box binary into the helper's fixed CORE path
    /// so TUN mode runs the version the user picked. The client owns the
    /// source path (in its app-data dir); since the trust model already grants
    /// the user control over the helper, this is just a privileged file copy
    /// the user couldn't otherwise do.
    InstallCore { path: String },
    /// ICMP round-trip to `host` (so the GUI can ping locations even when the
    /// user's ISP blocks raw TCP to those IPs but lets ICMP through).
    PingHost {
        host: String,
        #[serde(default = "default_ping_timeout")]
        timeout_ms: u32,
    },
    /// TCP-connect RTT to `host:port`, source-bound via SO_BINDTODEVICE to the
    /// user's physical interface so the probe bypasses sing-box's
    /// auto_route/auto_redirect nftables fast-path even while the tunnel is up.
    /// Plain SO_BIND to a phys-iface address isn't enough — auto_redirect
    /// catches at the nftables layer regardless of source IP. SO_BINDTODEVICE
    /// requires CAP_NET_RAW, which is why this lives in the helper.
    TcpPingHost {
        host: String,
        port: u16,
        #[serde(default = "default_ping_timeout")]
        timeout_ms: u32,
    },
}

fn default_ping_timeout() -> u32 { 2000 }

const KS_TABLE: &str = "aegis_ks";

#[derive(Serialize)]
struct Response {
    ok: bool,
    state: String,
    pid: Option<u32>,
    error: Option<String>,
    /// Result for PingHost / generic numeric returns. None for other requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    rtt_ms: Option<u32>,
}

impl Response {
    fn ok(state: &str, pid: Option<u32>) -> Self {
        Response { ok: true, state: state.into(), pid, error: None, rtt_ms: None }
    }
    fn err(state: &str, msg: String) -> Self {
        Response { ok: false, state: state.into(), pid: None, error: Some(msg), rtt_ms: None }
    }
    fn rtt(ms: u32) -> Self {
        Response { ok: true, state: "ok".into(), pid: None, error: None, rtt_ms: Some(ms) }
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
        // Defense in depth: if the daemon was restarted while sing-box was
        // running (so self.child is None but a sing-box owned by us is still
        // alive in /usr/local/lib/aegisvpn/sing-box), sweep it up too.
        // Otherwise disconnect would silently leave the tunnel up.
        kill_stray_core();
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
        // Stop the old core gracefully (so it removes its routes).
        if let Some(mut c) = self.child.take() {
            terminate_gracefully(&mut c);
        }

        if !std::path::Path::new(CORE).exists() {
            remove_killswitch();
            return Err(format!("sing-box core not installed at {CORE}"));
        }

        // Tear down any pre-existing killswitch BEFORE we resolve, otherwise
        // a stale ruleset from a previous session (different server IP, or
        // sing-box died leaving it in place) would block the DNS query and
        // resolve_ips would come back empty. A brief leak window between
        // teardown and re-apply is the trade-off for being able to reconnect
        // at all — the previous "always-on" guarantee was illusory anyway
        // since the rules might have been pinned to an old IP.
        remove_killswitch();

        if killswitch {
            let ips = resolve_ips(server);
            if ips.is_empty() {
                return Err(format!(
                    "cannot resolve proxy host '{server}' — DNS is unavailable. \
                     Restart your system resolver (e.g. systemctl restart \
                     systemd-resolved) or disable the killswitch in Settings, \
                     then retry."
                ));
            }
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
/// Resolve the proxy host to one or more IPs for the killswitch allow-list.
///
/// Tries an IP-literal parse first (no DNS, deterministic), then falls back to
/// the system resolver. Returning empty is a real failure — apply_killswitch
/// would lock the user out — so the caller checks for that explicitly.
fn resolve_ips(host: &str) -> Vec<std::net::IpAddr> {
    use std::net::ToSocketAddrs;
    use std::str::FromStr;
    if host.is_empty() {
        return Vec::new();
    }
    if let Ok(ip) = std::net::IpAddr::from_str(host) {
        return vec![ip];
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
    // `counter` on every rule so `nft list table inet aegis_ks` shows which
    // rule each packet hit. The final `counter drop` catches everything that
    // matched none of the allow-rules — its counter tells us exactly how
    // many bytes the killswitch is silently eating.
    r.push_str("    oifname \"lo\" counter accept\n");
    r.push_str("    oifname \"aegis0\" counter accept\n");
    r.push_str("    ct state established,related counter accept\n");
    // sing-box's auto_redirect mode TPROXY-redirects TCP to a local listener
    // (e.g. 127.0.0.1:46485). The kernel routes those packets out a real WAN
    // interface (enp11s0/wlan0) for arcane policy-routing reasons, so the
    // `oifname lo` rule above misses them and the final drop eats every
    // user TCP SYN. `fib daddr type local` accepts any packet whose
    // destination is an address local to this host, which is exactly what
    // we want here (loopback + any other interface address). Safe — these
    // packets are by definition not leaving the machine.
    r.push_str("    fib daddr type local counter accept\n");
    // sing-box marks every packet it originates itself (its own DNS lookups,
    // direct-outbound connections, etc.) with its routing mark — 0x2023/0x2024
    // are the standard values used by its auto_redirect + tproxy setup.
    // Without this, sing-box's own DNS/handshake traffic can hit the final
    // drop rule (it doesn't fit the lo/tun/established/explicit-allow buckets)
    // and the tunnel can never come up. Allow anything sing-box marked itself
    // — the trust boundary is "sing-box is allowed to talk to the network it
    // needs to". User-app traffic that somehow bypasses the TUN is still
    // dropped at the end.
    r.push_str("    meta mark & 0x0000ffff == 0x2023 counter accept\n");
    r.push_str("    meta mark & 0x0000ffff == 0x2024 counter accept\n");
    r.push_str("    ct mark & 0x0000ffff == 0x2023 counter accept\n");
    r.push_str("    ct mark & 0x0000ffff == 0x2024 counter accept\n");
    // DNS bootstrap to the resolver sing-box uses. Our config has two DNS
    // servers: `https://1.1.1.1` (DNS-over-HTTPS, port 443) and `udp://1.1.1.1`
    // (plain, port 53). Both must be reachable BEFORE the tunnel is up so the
    // initial server-name lookup can complete; otherwise the tunnel can never
    // come up because resolution itself is killswitched.
    r.push_str("    ip daddr 1.1.1.1 udp dport 53 counter accept\n");
    r.push_str("    ip daddr 1.1.1.1 tcp dport 53 counter accept\n");
    r.push_str("    ip daddr 1.1.1.1 tcp dport 443 counter accept\n");
    for ip in server_ips {
        match ip {
            std::net::IpAddr::V4(v4) => r.push_str(&format!("    ip daddr {v4} counter accept\n")),
            std::net::IpAddr::V6(v6) => r.push_str(&format!("    ip6 daddr {v6} counter accept\n")),
        }
    }
    if allow_lan {
        // Standard RFC1918 + link-local. 100.64.0.0/10 is CGNAT, which some
        // ISPs (mobile, satellite, T-Mobile home) put their customers on; without
        // it the user's own router/AP becomes unreachable.
        r.push_str(
            "    ip daddr { 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16, 100.64.0.0/10 } counter accept\n",
        );
        r.push_str("    ip6 daddr { fe80::/10, fc00::/7 } counter accept\n");
    }
    // Explicit terminal drop with a counter — what arrives here is what the
    // killswitch is killing. Diagnostic gold.
    //
    // `log prefix` writes one kernel-log line per dropped packet (visible via
    // `journalctl -k`), with the 5-tuple. When the killswitch breaks the
    // tunnel, this tells us EXACTLY which packets are being killed instead of
    // having to guess from a byte counter. Rate-limited so a flood can't
    // spam the journal. Stripped from production once we're confident the
    // ruleset is right.
    r.push_str("    limit rate 30/second log prefix \"aegis_ks_drop \" level info\n");
    r.push_str("    counter drop\n");
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

/// Kill any sing-box process running from the helper's CORE path that isn't
/// being tracked as our current child. Used to clean up orphans left behind
/// by a previous daemon instance (e.g. after a `systemctl restart`).
fn kill_stray_core() {
    // pkill -f matches the full command line; -SIGTERM is the default. We
    // could parse pidof / /proc ourselves, but pkill is universally available
    // and the exec path is rooted at /usr/local/lib/aegisvpn so we won't hit
    // unrelated sing-box installs.
    let _ = Command::new("pkill")
        .arg("-TERM")
        .arg("-f")
        .arg(CORE)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status();
    // Give SIGTERM a moment to land + take effect, then escalate any
    // survivors. Short total budget — we don't want disconnect to hang.
    std::thread::sleep(Duration::from_millis(400));
    let _ = Command::new("pkill")
        .arg("-KILL")
        .arg("-f")
        .arg(CORE)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status();
    // sing-box's TUN device sometimes outlives the process by a beat
    // (especially after SIGKILL — graceful shutdown unhooks routes, SIGKILL
    // doesn't). Force-remove it so the kernel routing table is clean. `ip
    // link delete` is a no-op if the device is already gone.
    let _ = Command::new("ip")
        .args(["link", "delete", "dev", "aegis0"])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status();
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

/// Copy a client-supplied sing-box binary to the helper's CORE path. We do
/// minimal validation (must exist, must be an ELF, sane size) — the rest of
/// the trust comes from the model: the helper accepts requests only from the
/// allowed UID, so the user is already authorised to drive what we run.
fn install_core(src: &str) -> Result<(), String> {
    let src_path = std::path::Path::new(src);
    let meta = std::fs::metadata(src_path).map_err(|e| format!("stat source: {e}"))?;
    if !meta.is_file() {
        return Err("source is not a regular file".into());
    }
    let size = meta.len();
    if !(1_000_000..=300_000_000).contains(&size) {
        return Err(format!("source size {size} bytes is out of range"));
    }
    let bytes = std::fs::read(src_path).map_err(|e| format!("read source: {e}"))?;
    // ELF magic on Linux. (Mach-O / PE binaries would fail here — fine, we
    // only ship sing-box for Linux through this path.)
    if bytes.len() < 4 || &bytes[..4] != b"\x7fELF" {
        return Err("source is not an ELF binary".into());
    }

    let dest = std::path::Path::new(CORE);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create CORE dir: {e}"))?;
    }
    // Write to a sibling temp then rename — atomic swap so a partial write
    // can never leave CORE in a half-state.
    let tmp = dest.with_extension("new");
    std::fs::write(&tmp, &bytes).map_err(|e| format!("write temp: {e}"))?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("chmod temp: {e}"))?;
    std::fs::rename(&tmp, dest).map_err(|e| format!("rename to CORE: {e}"))?;
    Ok(())
}

/// ICMP RTT via the system `ping` tool. Linux's ping doesn't need root when
/// `net.ipv4.ping_group_range` includes the user, but we're already in the
/// helper which runs as root, so it just works. Returns RTT in ms.
fn icmp_ping(host: &str, timeout_ms: u32) -> Result<u32, String> {
    if host.trim().is_empty() {
        return Err("empty host".into());
    }
    // Validate the host string before handing it to a subprocess — only
    // letters/digits/dots/colons/hyphens are valid for hostnames and IPs.
    if !host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-' | '_'))
    {
        return Err("invalid host".into());
    }
    let timeout_s = ((timeout_ms + 999) / 1000).max(1).min(10);
    let out = Command::new("ping")
        .arg("-n") // numeric, no rDNS
        .arg("-c").arg("1")
        .arg("-W").arg(timeout_s.to_string())
        .arg(host)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("spawn ping: {e}"))?;
    if !out.status.success() {
        return Err("ping failed".into());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // Parse "time=12.3 ms" out of the reply line.
    for line in text.lines() {
        if let Some(idx) = line.find("time=") {
            let rest = &line[idx + 5..];
            let num: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if let Ok(ms) = num.parse::<f64>() {
                return Ok(ms.round().max(1.0).min(u32::MAX as f64) as u32);
            }
        }
    }
    Err("no RTT in ping output".into())
}

/// First non-virtual interface with an IPv4 address. We bind probes to this
/// one via SO_BINDTODEVICE so they bypass sing-box's auto_route/auto_redirect
/// (which catches by destination, not source — plain SO_BIND to a phys-iface
/// IP isn't enough).
fn pick_physical_iface() -> Option<String> {
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
                let name = std::ffi::CStr::from_ptr(ifa.ifa_name).to_string_lossy().into_owned();
                let virt = name.starts_with("lo")
                    || name.starts_with("tun")
                    || name.starts_with("tap")
                    || name.starts_with("wg")
                    || name.starts_with("docker")
                    || name.starts_with("br-")
                    || name.starts_with("veth")
                    || name.starts_with("vmnet")
                    || name.starts_with("aegis");
                let sa = &*ifa.ifa_addr;
                if !virt && sa.sa_family as i32 == libc::AF_INET {
                    let sin = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                    let ip = std::net::Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
                    if !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified() {
                        found = Some(name);
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

/// TCP-connect RTT in ms, source-bound to a physical interface so the probe
/// doesn't get swallowed by the active VPN tunnel.
fn tcp_ping_bypass(host: &str, port: u16, timeout_ms: u32) -> Result<u32, String> {
    use socket2::{Domain, Protocol, SockAddr, Socket, Type};
    use std::net::{SocketAddr, ToSocketAddrs};
    use std::time::{Duration, Instant};

    if host.trim().is_empty() {
        return Err("empty host".into());
    }
    if !host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-' | '_'))
    {
        return Err("invalid host".into());
    }

    let dst: SocketAddr = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("resolve: {e}"))?
        .find(|a| a.is_ipv4())
        .ok_or_else(|| "no ipv4".to_string())?;

    let sock = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
        .map_err(|e| format!("socket: {e}"))?;

    // SO_MARK with sing-box's "tun bypass" mark (0x2024). The auto_redirect
    // nftables ruleset has a "meta mark 0x2024 return" exemption at the top of
    // both `output` and `output_udp_icmp` chains — packets with this mark
    // skip the TPROXY redirect and the killswitch's drop. Without this the
    // probe gets TPROXY'd into sing-box and the RTT reflects the tunnel.
    // Requires CAP_NET_ADMIN — which is why this lives in the helper.
    const SING_BOX_BYPASS_MARK: u32 = 0x2024;
    let _ = sock.set_mark(SING_BOX_BYPASS_MARK);

    // Belt-and-braces: also bind to the physical interface. Together with the
    // mark this guarantees the packet leaves the wire even if sing-box's mark
    // ever changes — the bind alone wouldn't be enough but it's cheap.
    if let Some(iface) = pick_physical_iface() {
        let _ = sock.bind_device(Some(iface.as_bytes()));
    }

    let timeout = Duration::from_millis(timeout_ms as u64);
    let started = Instant::now();
    sock.connect_timeout(&SockAddr::from(dst), timeout)
        .map_err(|e| format!("connect: {e}"))?;
    Ok(started.elapsed().as_millis().min(u32::MAX as u128) as u32)
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
            // Ping commands DON'T touch daemon state, so they must NOT take the
            // daemon lock — a TcpPingHost blocks for up to its timeout doing a
            // TCP connect, and holding the mutex across that serialized every
            // concurrent ping (locations pinged one-by-one instead of in
            // parallel). Handle them lock-free so the per-connection threads
            // actually run concurrently.
            Ok(Request::TcpPingHost { host, port, timeout_ms }) => {
                match tcp_ping_bypass(&host, port, timeout_ms) {
                    Ok(ms) => Response::rtt(ms),
                    Err(e) => Response::err("unreachable", e),
                }
            }
            Ok(Request::PingHost { host, timeout_ms }) => match icmp_ping(&host, timeout_ms) {
                Ok(ms) => Response::rtt(ms),
                Err(e) => Response::err("unreachable", e),
            },
            Ok(Request::InstallCore { path }) => match install_core(&path) {
                Ok(()) => Response::ok("installed", None),
                Err(e) => Response::err("unknown", e),
            },
            // State-touching commands: take the lock for the minimum needed.
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
                    // ping / install handled lock-free above; unreachable here.
                    _ => Response::err("unknown", "unhandled request".into()),
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
    // Same logic for sing-box: if the previous helper instance was killed
    // (systemctl restart) and the cgroup teardown didn't catch its sing-box
    // child, we'd start up with a stale tunnel still active and no way to
    // stop it (since the new instance has child: None). Sweep on boot.
    kill_stray_core();

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

//! sing-box core management.
//!
//! Versions are cached side-by-side under `app_data/core/versions/<tag>/sing-box`
//! and `core/active.txt` records which one the rest of the app should use. The
//! user downloads + keeps as many versions as they want, then activates one with
//! a single click — no re-download to switch. Activating also pushes the binary
//! to the helper so TUN mode uses the same version as proxy mode.
//!
//! Downloads stream chunks + emit `core://progress` events so the UI can render
//! a real progress bar (bytes, total, speed) instead of a spinner.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

const REPO: &str = "SagerNet/sing-box";
const ACTIVE_FILE: &str = "active.txt";

#[derive(Serialize, Clone)]
pub struct InstalledVersion {
    pub tag: String,
    pub active: bool,
}

#[derive(Serialize)]
pub struct CoreInfo {
    /// All locally-cached versions, newest first; `active` flags which one
    /// the app (and helper, once synced) is actually running.
    pub installed: Vec<InstalledVersion>,
    /// Tag of the active version, or null when none installed.
    pub active: Option<String>,
    /// Latest version from GitHub releases, or null when the check failed.
    pub latest: Option<String>,
    /// True iff `latest` is set and differs from `active` (or none active).
    pub has_update: bool,
}

#[derive(Serialize, Clone)]
pub struct CoreProgress {
    /// Version tag this progress event is for.
    pub tag: String,
    pub downloaded: u64,
    /// Total size from Content-Length, or 0 if the server didn't send it.
    pub total: u64,
    /// Bytes-per-second over the last sample window.
    pub speed_bps: u64,
}

// --- paths -----------------------------------------------------------------

fn core_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?
        .join("core");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create core dir: {e}"))?;
    Ok(dir)
}

fn versions_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let d = core_dir(app)?.join("versions");
    std::fs::create_dir_all(&d).map_err(|e| format!("create versions dir: {e}"))?;
    Ok(d)
}

fn binary_name() -> &'static str {
    if cfg!(windows) { "sing-box.exe" } else { "sing-box" }
}

/// Where the binary for a specific tag lives (may not exist yet).
fn version_binary(app: &AppHandle, tag: &str) -> Result<PathBuf, String> {
    Ok(versions_dir(app)?.join(strip_v(tag)).join(binary_name()))
}

fn strip_v(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

/// The currently active version's binary path; errors if no version is active
/// (caller should prompt the user to install one).
pub fn binary_path(app: &AppHandle) -> Result<PathBuf, String> {
    let active = active_tag(app)
        .ok_or_else(|| "no sing-box core installed (Settings → VPN core)".to_string())?;
    let bin = version_binary(app, &active)?;
    if !bin.exists() {
        return Err(format!(
            "active core version {active} is missing on disk — pick another in Settings"
        ));
    }
    Ok(bin)
}

/// Tag currently selected as active (whatever's in `active.txt`).
pub fn active_tag(app: &AppHandle) -> Option<String> {
    let dir = core_dir(app).ok()?;
    let v = std::fs::read_to_string(dir.join(ACTIVE_FILE)).ok()?;
    let v = v.trim().to_string();
    if v.is_empty() { None } else { Some(v) }
}

fn write_active(app: &AppHandle, tag: &str) -> Result<(), String> {
    let dir = core_dir(app)?;
    std::fs::write(dir.join(ACTIVE_FILE), strip_v(tag))
        .map_err(|e| format!("write active.txt: {e}"))
}

fn clear_active(app: &AppHandle) -> Result<(), String> {
    let dir = core_dir(app)?;
    let p = dir.join(ACTIVE_FILE);
    if p.exists() {
        std::fs::remove_file(p).map_err(|e| format!("remove active.txt: {e}"))?;
    }
    Ok(())
}

/// One-time migration of the old single-binary layout
/// (`core/sing-box` + `core/version.txt`) into the new per-version cache
/// (`core/versions/<tag>/sing-box` + `core/active.txt`). Idempotent: a no-op
/// once the old files are gone.
fn migrate_legacy_layout(app: &AppHandle) {
    let Ok(dir) = core_dir(app) else { return };
    let old_bin = dir.join(binary_name());
    let old_ver_file = dir.join("version.txt");
    if !old_bin.exists() {
        return;
    }
    let ver = std::fs::read_to_string(&old_ver_file)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "legacy".to_string());

    if let Ok(target) = version_binary(app, &ver) {
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if !target.exists() && std::fs::rename(&old_bin, &target).is_err() {
            // Cross-device move? fall back to copy + remove.
            if std::fs::copy(&old_bin, &target).is_ok() {
                let _ = std::fs::remove_file(&old_bin);
            }
        }
        if active_tag(app).is_none() {
            let _ = write_active(app, &ver);
        }
    }
    let _ = std::fs::remove_file(&old_ver_file);
}

/// All tags with a binary on disk, newest-first by simple version ordering.
fn installed_tags(app: &AppHandle) -> Vec<String> {
    migrate_legacy_layout(app);
    let Ok(dir) = versions_dir(app) else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut tags: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if e.path().join(binary_name()).exists() {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    tags.sort_by(|a, b| version_cmp(b, a));
    tags
}

/// Compare semver-ish tags ("1.13.0" vs "1.12.4"). Falls back to lexical for
/// anything that doesn't parse.
fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |s: &str| -> Vec<u32> {
        strip_v(s)
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.parse::<u32>().ok())
            .collect()
    };
    parts(a).cmp(&parts(b))
}

// --- GitHub API ------------------------------------------------------------

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("AegisVPN/0.1 (core-updater)")
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("http client: {e}"))
}

async fn fetch_latest_release() -> Result<serde_json::Value, String> {
    let client = http_client()?;
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("release request: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API HTTP {}", resp.status()));
    }
    let text = resp.text().await.map_err(|e| format!("release body: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("release json: {e}"))
}

async fn fetch_release_by_tag(tag: &str) -> Result<serde_json::Value, String> {
    let client = http_client()?;
    let tag = if tag.starts_with('v') { tag.to_string() } else { format!("v{tag}") };
    let url = format!("https://api.github.com/repos/{REPO}/releases/tags/{tag}");
    let resp = client.get(url).send().await.map_err(|e| format!("release request: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API HTTP {}", resp.status()));
    }
    let text = resp.text().await.map_err(|e| format!("release body: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("release json: {e}"))
}

fn version_from_tag(release: &serde_json::Value) -> Option<String> {
    release.get("tag_name").and_then(|t| t.as_str()).map(|t| strip_v(t).to_string())
}

async fn latest_version() -> Result<String, String> {
    let release = fetch_latest_release().await?;
    version_from_tag(&release).ok_or_else(|| "no tag_name in release".to_string())
}

#[derive(Serialize)]
pub struct CoreRelease {
    pub tag: String,
    pub name: String,
    pub date: Option<String>,
    pub prerelease: bool,
}

#[tauri::command]
pub async fn list_core_releases() -> Result<Vec<CoreRelease>, String> {
    let client = http_client()?;
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=30");
    let resp = client.get(url).send().await.map_err(|e| format!("releases request: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API HTTP {}", resp.status()));
    }
    let text = resp.text().await.map_err(|e| format!("releases body: {e}"))?;
    let arr: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("releases json: {e}"))?;
    Ok(arr
        .as_array()
        .ok_or("releases not an array")?
        .iter()
        .filter_map(|r| {
            let tag = r.get("tag_name")?.as_str()?.to_string();
            let name = r
                .get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| tag.clone());
            let date = r.get("published_at").and_then(|d| d.as_str()).map(|s| s.to_string());
            let prerelease = r.get("prerelease").and_then(|p| p.as_bool()).unwrap_or(false);
            Some(CoreRelease { tag, name, date, prerelease })
        })
        .collect())
}

// --- download + install ----------------------------------------------------

/// sing-box names release assets `sing-box-<ver>-<os>-<arch>.tar.gz`.
fn asset_suffix() -> Result<String, String> {
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "darwin",
        "windows" => "windows",
        other => return Err(format!("unsupported OS: {other}")),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => return Err(format!("unsupported arch: {other}")),
    };
    Ok(format!("-{os}-{arch}.tar.gz"))
}

/// Download the asset with progress events, verify its SHA256 against the
/// release's per-asset digest, and extract the binary into the per-tag dir.
async fn download_and_install(
    app: &AppHandle,
    tag: &str,
    release: &serde_json::Value,
) -> Result<(), String> {
    let suffix = asset_suffix()?;
    let asset = release
        .get("assets")
        .and_then(|a| a.as_array())
        .ok_or("release has no assets")?
        .iter()
        .find(|a| {
            a.get("name")
                .and_then(|n| n.as_str())
                .map(|name| name.ends_with(&suffix) && !name.contains("legacy"))
                .unwrap_or(false)
        })
        .ok_or_else(|| format!("no asset matching '*{suffix}'"))?;

    let url = asset
        .get("browser_download_url")
        .and_then(|u| u.as_str())
        .ok_or("asset has no download url")?
        .to_string();
    let total: u64 = asset.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
    let digest = asset
        .get("digest")
        .and_then(|d| d.as_str())
        .and_then(|d| d.strip_prefix("sha256:"))
        .map(|h| h.to_lowercase());

    let client = http_client()?;
    let resp = client.get(&url).send().await.map_err(|e| format!("download: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download HTTP {}", resp.status()));
    }

    let mut buf: Vec<u8> = Vec::with_capacity(total as usize);
    let mut stream = resp.bytes_stream();
    let mut last_emit = Instant::now();
    let mut window_start = Instant::now();
    let mut window_bytes: u64 = 0;
    let mut speed_bps: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("download chunk: {e}"))?;
        buf.extend_from_slice(&chunk);
        window_bytes += chunk.len() as u64;

        // Sample the speed every ~250 ms so the displayed number is steady.
        let now = Instant::now();
        if now.duration_since(window_start) >= Duration::from_millis(250) {
            let elapsed = now.duration_since(window_start).as_secs_f64();
            if elapsed > 0.0 {
                speed_bps = (window_bytes as f64 / elapsed) as u64;
            }
            window_start = now;
            window_bytes = 0;
        }

        // Throttle progress events to ~10/s — the UI doesn't need more.
        if now.duration_since(last_emit) >= Duration::from_millis(100) {
            let _ = app.emit(
                "core://progress",
                CoreProgress { tag: tag.to_string(), downloaded: buf.len() as u64, total, speed_bps },
            );
            last_emit = now;
        }
    }

    // Final 100% event so progress bars don't get stuck at 99%.
    let _ = app.emit(
        "core://progress",
        CoreProgress { tag: tag.to_string(), downloaded: buf.len() as u64, total: buf.len() as u64, speed_bps },
    );

    if let Some(expected) = digest {
        let actual = sha256_hex(&buf);
        if actual != expected {
            return Err(format!(
                "core checksum mismatch (expected {expected}, got {actual}) — refusing to install"
            ));
        }
    }

    let dest = version_binary(app, tag)?;
    std::fs::create_dir_all(dest.parent().unwrap())
        .map_err(|e| format!("create version dir: {e}"))?;
    extract_binary(&buf, &dest)?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Pull the `sing-box` binary out of the release tarball and write it to `dest`.
fn extract_binary(tar_gz: &[u8], dest: &PathBuf) -> Result<(), String> {
    use flate2::read::GzDecoder;
    let mut archive = tar::Archive::new(GzDecoder::new(tar_gz));
    let entries = archive.entries().map_err(|e| format!("tar: {e}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| format!("tar entry: {e}"))?;
        let path = entry.path().map_err(|e| format!("tar path: {e}"))?;
        let is_bin = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == binary_name())
            .unwrap_or(false);
        if is_bin {
            entry.unpack(dest).map_err(|e| format!("unpack: {e}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755));
            }
            return Ok(());
        }
    }
    Err("sing-box binary not found in archive".to_string())
}

// --- helper sync -----------------------------------------------------------

/// After activating a version, push the binary to the helper so TUN mode
/// (which runs sing-box as root via the helper) uses the same version. The
/// helper accepts a path inside the user's app-data dir and copies it to its
/// fixed root-owned location.
fn sync_helper(app: &AppHandle) -> Result<(), String> {
    use crate::vpn::helper_install_core;
    let bin = binary_path(app)?;
    helper_install_core(bin)
}

// --- public API ------------------------------------------------------------

#[tauri::command]
pub async fn core_info(app: AppHandle) -> CoreInfo {
    let active = active_tag(&app);
    let installed: Vec<InstalledVersion> = installed_tags(&app)
        .into_iter()
        .map(|tag| InstalledVersion {
            active: active.as_deref() == Some(tag.as_str()),
            tag,
        })
        .collect();
    let latest = latest_version().await.ok();
    let has_update = match (&active, &latest) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(a), Some(l)) => version_cmp(l, a) == std::cmp::Ordering::Greater,
    };
    CoreInfo { installed, active, latest, has_update }
}

/// Download `version` (or the latest release when null) into the per-tag cache.
/// If nothing is active yet, automatically activate the new version. Emits
/// `core://progress` while downloading. Returns the installed tag.
#[tauri::command]
pub async fn core_install(app: AppHandle, version: Option<String>) -> Result<String, String> {
    let release = match version {
        Some(t) => fetch_release_by_tag(&t).await?,
        None => fetch_latest_release().await?,
    };
    let tag = version_from_tag(&release).ok_or("no version in release")?;

    download_and_install(&app, &tag, &release).await?;

    // First-install convenience: if nothing was active, make this one active
    // (and sync it to the helper) so the user can immediately connect.
    if active_tag(&app).is_none() {
        write_active(&app, &tag)?;
        let _ = sync_helper(&app);
    } else if active_tag(&app).as_deref() == Some(tag.as_str()) {
        // Re-install of the currently active version: refresh the helper copy.
        let _ = sync_helper(&app);
    }
    Ok(tag)
}

/// Switch the active version to `tag` (must already be downloaded) and push
/// the binary to the helper so TUN mode picks it up.
#[tauri::command]
pub async fn core_activate(app: AppHandle, tag: String) -> Result<(), String> {
    let bin = version_binary(&app, &tag)?;
    if !bin.exists() {
        return Err(format!("version {tag} isn't downloaded"));
    }
    write_active(&app, &tag)?;
    // Best-effort helper sync — when the helper isn't installed yet, proxy
    // mode still works; activation succeeds either way.
    let _ = sync_helper(&app);
    Ok(())
}

/// Delete a cached version. Deleting the active one is allowed: callers are
/// expected to disconnect first (the frontend does), and we clear active.txt
/// so the next connect attempt errors clearly with "no core installed".
#[tauri::command]
pub async fn core_uninstall(app: AppHandle, tag: String) -> Result<(), String> {
    let was_active = active_tag(&app).as_deref() == Some(strip_v(&tag));
    let dir = versions_dir(&app)?.join(strip_v(&tag));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("remove version dir: {e}"))?;
    }
    if was_active {
        let _ = clear_active(&app);
    }
    // If somehow we end up with nothing installed at all, clear the marker
    // defensively so core_info() reports a clean "no core" state.
    if installed_tags(&app).is_empty() {
        let _ = clear_active(&app);
    }
    Ok(())
}

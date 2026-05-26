//! sing-box core management: detect, download, and auto-update the sing-box
//! binary from the SagerNet/sing-box GitHub releases. The binary lives under
//! the app data dir so it can be updated without touching system files.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager};

const REPO: &str = "SagerNet/sing-box";
const VERSION_FILE: &str = "version.txt";

#[derive(Serialize)]
pub struct CoreInfo {
    /// Installed core version (e.g. "1.11.0"), or null when not installed.
    pub installed: Option<String>,
    /// Latest release version, or null when the check failed (offline, etc.).
    pub latest: Option<String>,
    /// True when an install/update should be offered.
    pub has_update: bool,
}

fn core_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?
        .join("core");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create core dir: {e}"))?;
    Ok(dir)
}

fn binary_name() -> &'static str {
    if cfg!(windows) {
        "sing-box.exe"
    } else {
        "sing-box"
    }
}

/// Path to the managed sing-box binary (may not exist yet).
pub fn binary_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(core_dir(app)?.join(binary_name()))
}

/// The installed version, read from the marker file we write on install.
pub fn installed_version(app: &AppHandle) -> Option<String> {
    let dir = core_dir(app).ok()?;
    if !dir.join(binary_name()).exists() {
        return None;
    }
    let v = std::fs::read_to_string(dir.join(VERSION_FILE)).ok()?;
    let v = v.trim().to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

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
    // Windows ships .zip; we currently support the .tar.gz platforms.
    Ok(format!("-{os}-{arch}.tar.gz"))
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("AegisVPN/0.1 (core-updater)")
        .timeout(std::time::Duration::from_secs(60))
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

fn version_from_tag(release: &serde_json::Value) -> Option<String> {
    release
        .get("tag_name")
        .and_then(|t| t.as_str())
        .map(|t| t.trim_start_matches('v').to_string())
}

/// Latest released version string, or an error when unreachable.
pub async fn latest_version() -> Result<String, String> {
    let release = fetch_latest_release().await?;
    version_from_tag(&release).ok_or_else(|| "no tag_name in release".to_string())
}

/// Fetch a specific release by tag (e.g. "v1.13.0").
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

#[derive(Serialize)]
pub struct CoreRelease {
    pub tag: String,
    pub name: String,
    pub date: Option<String>,
    pub prerelease: bool,
}

/// Recent sing-box releases (newest first), for the version picker.
#[tauri::command]
pub async fn list_core_releases() -> Result<Vec<CoreRelease>, String> {
    let client = http_client()?;
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=30");
    let resp = client.get(url).send().await.map_err(|e| format!("releases request: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API HTTP {}", resp.status()));
    }
    let text = resp.text().await.map_err(|e| format!("releases body: {e}"))?;
    let arr: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("releases json: {e}"))?;
    let releases: Vec<CoreRelease> = arr
        .as_array()
        .ok_or("releases not an array")?
        .iter()
        .filter_map(|r| {
            let tag = r.get("tag_name")?.as_str()?.to_string();
            let name = r.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()).unwrap_or_else(|| tag.clone());
            let date = r.get("published_at").and_then(|d| d.as_str()).map(|s| s.to_string());
            let prerelease = r.get("prerelease").and_then(|p| p.as_bool()).unwrap_or(false);
            Some(CoreRelease { tag, name, date, prerelease })
        })
        .collect();
    Ok(releases)
}

/// Download + extract the latest sing-box (or a specific tag) for this
/// platform. Returns the installed version on success.
pub async fn install_version(app: &AppHandle, tag: Option<String>) -> Result<String, String> {
    let release = match tag {
        Some(t) => fetch_release_by_tag(&t).await?,
        None => fetch_latest_release().await?,
    };
    let version = version_from_tag(&release).ok_or("no version in release")?;
    let suffix = asset_suffix()?;

    let assets = release
        .get("assets")
        .and_then(|a| a.as_array())
        .ok_or("release has no assets")?;
    let asset = assets
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
    // GitHub exposes a per-asset content digest ("sha256:<hex>"). The core is
    // later run as root by the helper, so verify the download against it.
    let digest = asset
        .get("digest")
        .and_then(|d| d.as_str())
        .and_then(|d| d.strip_prefix("sha256:"))
        .map(|h| h.to_lowercase());

    let client = http_client()?;
    let bytes = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("download: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("download body: {e}"))?;

    if let Some(expected) = digest {
        let actual = sha256_hex(&bytes);
        if actual != expected {
            return Err(format!(
                "core checksum mismatch (expected {expected}, got {actual}) — refusing to install"
            ));
        }
    }

    let bin = binary_path(app)?;
    extract_binary(&bytes, &bin)?;
    std::fs::write(core_dir(app)?.join(VERSION_FILE), &version)
        .map_err(|e| format!("write version: {e}"))?;
    Ok(version)
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
            entry
                .unpack(dest)
                .map_err(|e| format!("unpack: {e}"))?;
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

// --- Tauri commands ---------------------------------------------------------

#[tauri::command]
pub async fn core_info(app: AppHandle) -> CoreInfo {
    let installed = installed_version(&app);
    let latest = latest_version().await.ok();
    let has_update = match (&installed, &latest) {
        (_, None) => false,                       // can't check → don't nag
        (None, Some(_)) => true,                  // not installed → offer install
        (Some(i), Some(l)) => i != l,             // differs → offer update
    };
    CoreInfo { installed, latest, has_update }
}

#[tauri::command]
pub async fn core_install(app: AppHandle, version: Option<String>) -> Result<String, String> {
    install_version(&app, version).await
}

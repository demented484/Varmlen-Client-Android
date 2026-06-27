//! System tray + native Linux autostart.
//!
//! The tray keeps Varmlen running with no window: closing the window hides it
//! here (the VPN stays up), and Quit — the only path that tears the tunnel
//! down — lives in the tray menu. Autostart is a `~/.config/autostart` entry we
//! write/remove ourselves (Linux-only target), with a `--minimized` arg so the
//! login launch can start straight to the tray.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

/// Build the tray icon + menu. Left-click shows the window; the menu has the
/// connect/disconnect toggle, Open, and Quit.
pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Connect / Disconnect", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Open Varmlen", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&toggle, &sep, &show, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Varmlen")
        .on_menu_event(|app, event| match event.id.as_ref() {
            // Connecting needs the current server + split config, which live in
            // the frontend — signal it rather than reimplement that here.
            "toggle" => {
                let _ = app.emit("tray://toggle", ());
            }
            "show" => show_main(app),
            "quit" => quit_app(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Show + focus the main window (from the tray).
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Whether closing the window hides to the tray (true) or fully quits (false).
/// Mirrors the user's setting; the frontend pushes it via `set_close_to_tray`.
static CLOSE_TO_TRAY: AtomicBool = AtomicBool::new(true);

pub fn close_to_tray() -> bool {
    CLOSE_TO_TRAY.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn set_close_to_tray(enabled: bool) {
    CLOSE_TO_TRAY.store(enabled, Ordering::Relaxed);
}

/// Tear the tunnel down, then exit. The only clean way out of the app.
pub(crate) fn quit_app(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::vpn::vpn_disconnect(app.clone()).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        app.exit(0);
    });
}

/// Reflect the connection status in the tray tooltip. Called from the frontend
/// (which owns the localized status text) whenever the status changes.
#[tauri::command]
pub fn set_tray_status(app: AppHandle, status_label: String) {
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(format!("Varmlen — {status_label}")));
    }
}

// --- native Linux autostart -------------------------------------------------

#[derive(Serialize)]
pub struct AutostartStatus {
    pub enabled: bool,
    pub minimized: bool,
}

fn autostart_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("autostart").join("varmlen.desktop"))
}

#[tauri::command]
pub fn autostart_status() -> AutostartStatus {
    match autostart_path().and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(c) => AutostartStatus {
            enabled: true,
            minimized: c.contains("--minimized"),
        },
        None => AutostartStatus {
            enabled: false,
            minimized: false,
        },
    }
}

#[tauri::command]
pub fn set_autostart(enabled: bool, minimized: bool) -> Result<(), String> {
    let path = autostart_path().ok_or("no config dir")?;
    if !enabled {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| format!("current exe: {e}"))?;
    let exec = if minimized {
        format!("{} --minimized", exe.display())
    } else {
        exe.display().to_string()
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("autostart dir: {e}"))?;
    }
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Varmlen\nGenericName=VPN Client\nIcon=varmlen\nExec={exec}\nTerminal=false\nCategories=Network;Security;\nX-GNOME-Autostart-enabled=true\n"
    );
    std::fs::write(&path, entry).map_err(|e| format!("write autostart: {e}"))?;
    Ok(())
}

/// True when launched from the autostart entry's `--minimized` exec — start
/// straight to the tray with no window.
pub fn launched_minimized() -> bool {
    std::env::args().any(|a| a == "--minimized")
}

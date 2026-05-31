//! System tray: show window, prevent sleep, restart, quit.

use std::{
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use tray_icon::{
    Icon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu},
};

use crate::power::PreventSleepMode;

/// Tray menu actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    SetPreventSleep(PreventSleepMode),
    Restart,
    Quit,
}

const MENU_SHOW: &str = "qingqi.tray.show";
const MENU_SLEEP_DISABLED: &str = "qingqi.tray.sleep.disabled";
const MENU_SLEEP_ALWAYS: &str = "qingqi.tray.sleep.always";
const MENU_SLEEP_PLUGGED: &str = "qingqi.tray.sleep.plugged";
const MENU_RESTART: &str = "qingqi.tray.restart";
const MENU_QUIT: &str = "qingqi.tray.quit";

#[cfg(any(target_os = "macos", target_os = "windows"))]
static TRAY_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Install tray icon and menu. Call on the main thread after the event loop runs.
pub fn install_tray(mode: PreventSleepMode) -> Result<(), String> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let menu = build_menu(mode)?;
        let icon = default_icon()?;
        let mut builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("Qingqi");

        #[cfg(target_os = "macos")]
        {
            builder = builder.with_icon_as_template(true);
        }

        let tray = builder
            .with_icon(icon)
            .build()
            .map_err(|error| error.to_string())?;

        // Drop the previous tray icon (replaces it in the system menu bar).
        replace_tray(tray);
        TRAY_INSTALLED.store(true, Ordering::SeqCst);
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = mode;
        Err(String::from("system tray not supported on this platform"))
    }
}

/// Rebuild the tray menu with updated sleep mode check marks.
pub fn rebuild_menu(mode: PreventSleepMode) -> Result<(), String> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let menu = build_menu(mode)?;
        with_tray(|tray| {
            tray.set_menu(Some(Box::new(menu)));
        });
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = mode;
        Ok(())
    }
}

/// Poll tray click and menu events. Returns pending actions.
pub fn poll_actions() -> Vec<TrayAction> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

        let mut actions = Vec::new();

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                actions.push(TrayAction::Show);
            }
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let Some(action) = action_for_menu_id(event.id().as_ref()) else {
                continue;
            };
            actions.push(action);
        }

        actions
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}

/// Launch a new process; caller exits the current one.
pub fn relaunch() {
    let Ok(exe) = std::env::current_exe() else {
        tracing::warn!("restart failed: cannot resolve current executable");
        return;
    };

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(280));
        if let Err(error) = Command::new(&exe).spawn() {
            tracing::warn!(error = %error, "restart spawn failed");
        }
    });
}

// ── Internals ──

#[cfg(any(target_os = "macos", target_os = "windows"))]
use tray_icon::TrayIcon;

/// Stored tray icon. Only accessed from the main thread (GPUI event loop).
#[cfg(any(target_os = "macos", target_os = "windows"))]
static mut CURRENT_TRAY: Option<TrayIcon> = None;

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn replace_tray(tray: TrayIcon) {
    unsafe {
        CURRENT_TRAY = Some(tray);
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn with_tray(f: impl FnOnce(&TrayIcon)) {
    unsafe {
        if let Some(ref tray) = CURRENT_TRAY {
            f(tray);
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn build_menu(mode: PreventSleepMode) -> Result<Menu, String> {
    let menu = Menu::new();

    let show = MenuItem::with_id(MenuId::new(MENU_SHOW), "显示界面", true, None);
    menu.append(&show).map_err(|error| error.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|error| error.to_string())?;

    // ── Prevent Sleep submenu ──
    let sleep_sub = Submenu::new("防止休眠", true);

    let disabled = CheckMenuItem::with_id(
        MenuId::new(MENU_SLEEP_DISABLED),
        "不开启",
        true,
        mode == PreventSleepMode::Disabled,
        None,
    );
    let always = CheckMenuItem::with_id(
        MenuId::new(MENU_SLEEP_ALWAYS),
        "始终开启",
        true,
        mode == PreventSleepMode::AlwaysOn,
        None,
    );
    let plugged = CheckMenuItem::with_id(
        MenuId::new(MENU_SLEEP_PLUGGED),
        "仅接入电源开启",
        true,
        mode == PreventSleepMode::WhenPluggedIn,
        None,
    );

    sleep_sub
        .append(&disabled)
        .map_err(|error| error.to_string())?;
    sleep_sub
        .append(&always)
        .map_err(|error| error.to_string())?;
    sleep_sub
        .append(&plugged)
        .map_err(|error| error.to_string())?;

    menu.append(&sleep_sub).map_err(|error| error.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|error| error.to_string())?;

    let restart = MenuItem::with_id(MenuId::new(MENU_RESTART), "重启", true, None);
    let quit = MenuItem::with_id(MenuId::new(MENU_QUIT), "退出", true, None);

    menu.append(&restart).map_err(|error| error.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|error| error.to_string())?;
    menu.append(&quit).map_err(|error| error.to_string())?;
    Ok(menu)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn default_icon() -> Result<Icon, String> {
    if let Some(icon) = load_tray_svg_icon() {
        return Ok(icon);
    }

    const SIZE: u32 = 22;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];

    fn right_edge(px: f32, py: f32) -> f32 {
        let row = py as i32;
        let b: f32 = match row {
            0 => 10.5,
            1 => 9.5,
            2 => 8.5,
            3 => 7.5,
            4..=12 => 5.5,
            13 => 7.5,
            14 => 6.5,
            15 => 5.5,
            16 => 4.5,
            17 => 3.5,
            18 => 3.5,
            19 => 2.5,
            _ => 0.0,
        };
        let f: f32 = match row {
            16 => 10.5,
            17 => 10.5,
            18 => 9.5,
            _ => b,
        };
        f.max(b) - px
    }

    fn left_edge(px: f32, py: f32) -> f32 {
        let row = py as i32;
        let b: f32 = match row {
            0 => 10.5,
            1 => 11.5,
            2 => 12.5,
            3 => 13.5,
            4..=12 => 15.5,
            13 => 13.5,
            14 => 14.5,
            15 => 15.5,
            16 => 16.5,
            17 => 17.5,
            18 => 17.5,
            19 => 18.5,
            _ => 0.0,
        };
        let f: f32 = match row {
            16 => 10.5,
            17 => 10.5,
            18 => 11.5,
            _ => b,
        };
        px - f.min(b)
    }

    for y in 0..SIZE {
        for x in 0..SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let d = left_edge(px, py).min(right_edge(px, py));
            let alpha = (1.0 - d.max(0.0).min(1.0)).max(0.0).min(1.0);
            let idx = ((y * SIZE + x) * 4) as usize;
            rgba[idx] = 255;
            rgba[idx + 1] = 255;
            rgba[idx + 2] = 255;
            rgba[idx + 3] = (alpha * 255.0) as u8;
        }
    }

    Icon::from_rgba(rgba, SIZE, SIZE).map_err(|error| error.to_string())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn load_tray_svg_icon() -> Option<Icon> {
    // macOS 菜单栏标准逻辑尺寸为 22pt，使用 2x 位图保证 Retina 清晰
    const SIZE: u32 = 44;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../qingqi/assets/tray-icon.svg");
    let rgba = crate::svg_icon::rasterize_path(path.as_path(), SIZE).ok()?;
    icon_from_rgba_template(rgba, SIZE, SIZE).ok()
}

/// 转为模板图标：黑色剪影 + alpha（macOS 会根据深浅色菜单栏自动反转）。
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn icon_from_rgba_template(rgba: Vec<u8>, width: u32, height: u32) -> Result<Icon, String> {
    let mut out = rgba;
    for chunk in out.chunks_exact_mut(4) {
        let alpha = chunk[3];
        chunk[0] = 0;
        chunk[1] = 0;
        chunk[2] = 0;
        chunk[3] = alpha;
    }
    Icon::from_rgba(out, width, height).map_err(|error| error.to_string())
}

fn action_for_menu_id(id: &str) -> Option<TrayAction> {
    match id {
        MENU_SHOW => Some(TrayAction::Show),
        MENU_SLEEP_DISABLED => Some(TrayAction::SetPreventSleep(PreventSleepMode::Disabled)),
        MENU_SLEEP_ALWAYS => Some(TrayAction::SetPreventSleep(PreventSleepMode::AlwaysOn)),
        MENU_SLEEP_PLUGGED => Some(TrayAction::SetPreventSleep(PreventSleepMode::WhenPluggedIn)),
        MENU_RESTART => Some(TrayAction::Restart),
        MENU_QUIT => Some(TrayAction::Quit),
        _ => None,
    }
}

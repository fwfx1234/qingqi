//! 系统托盘：显示界面、重启、退出。

use std::{
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use tray_icon::{
    Icon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
};

/// 托盘菜单触发的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    Restart,
    Quit,
}

const MENU_SHOW: &str = "qingqi.tray.show";
const MENU_RESTART: &str = "qingqi.tray.restart";
const MENU_QUIT: &str = "qingqi.tray.quit";

#[cfg(any(target_os = "macos", target_os = "windows"))]
static TRAY_INSTALLED: AtomicBool = AtomicBool::new(false);

/// 在主线程、事件循环已运行后创建托盘图标与菜单。
pub fn install_tray() -> Result<(), String> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        if TRAY_INSTALLED.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let menu = build_menu()?;
        let icon = default_icon()?;
        let mut builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("Qingqi");

        #[cfg(target_os = "macos")]
        {
            builder = builder.with_icon_as_template(false);
        }

        let tray = builder
            .with_icon(icon)
            .build()
            .map_err(|error| error.to_string())?;

        // `TrayIcon` 非 `Sync`，常驻进程内由系统持有，避免放入全局静态。
        std::mem::forget(tray);
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(String::from("当前平台未实现系统托盘"))
    }
}

/// 轮询托盘点击与菜单事件，返回待处理动作（可一次取出多个）。
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

        return actions;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}

/// 启动新进程后由调用方退出当前进程。
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn build_menu() -> Result<Menu, String> {
    let menu = Menu::new();
    let show = MenuItem::with_id(MenuId::new(MENU_SHOW), "显示界面", true, None);
    let restart = MenuItem::with_id(MenuId::new(MENU_RESTART), "重启", true, None);
    let quit = MenuItem::with_id(MenuId::new(MENU_QUIT), "退出", true, None);

    menu.append(&show).map_err(|error| error.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|error| error.to_string())?;
    menu.append(&restart).map_err(|error| error.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|error| error.to_string())?;
    menu.append(&quit).map_err(|error| error.to_string())?;
    Ok(menu)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn default_icon() -> Result<Icon, String> {
    if let Some(icon) = load_asset_icon("tray_rocket.png") {
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
fn load_asset_icon(name: &str) -> Option<Icon> {
    for path in crate::app::assets::candidates(name) {
        match icon_from_png(&path) {
            Ok(icon) => return Some(icon),
            Err(error) => {
                tracing::debug!(path = %path.display(), error = %error, "tray icon load failed")
            }
        }
    }
    None
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn icon_from_png(path: &std::path::Path) -> Result<Icon, String> {
    let mut image = image::open(path)
        .map_err(|error| error.to_string())?
        .into_rgba8();
    let (width, height) = image.dimensions();
    for pixel in image.pixels_mut() {
        let alpha = pixel.0[3];
        pixel.0 = [255, 255, 255, alpha];
    }
    Icon::from_rgba(image.into_raw(), width, height).map_err(|error| error.to_string())
}

fn action_for_menu_id(id: &str) -> Option<TrayAction> {
    match id {
        MENU_SHOW => Some(TrayAction::Show),
        MENU_RESTART => Some(TrayAction::Restart),
        MENU_QUIT => Some(TrayAction::Quit),
        _ => None,
    }
}

//! System tray: show window, prevent sleep, restart, quit.

use std::{
    cell::RefCell,
    collections::HashMap,
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TrayItemId(String);

impl TrayItemId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn tray_icon_id(&self) -> String {
        format!("qingqi.tray.item.{}", self.0)
    }
}

impl From<&str> for TrayItemId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TrayItemId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayItemIcon {
    None,
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayItemSpec {
    pub id: TrayItemId,
    pub icon: TrayItemIcon,
    pub title: String,
    pub tooltip: String,
    pub menu: Vec<String>,
    pub priority: i32,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayItemState {
    pub spec: TrayItemSpec,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TrayIconAction {
    Main,
    Item(TrayItemClick),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayMouseButton {
    Left,
    Right,
    Middle,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayMouseButtonState {
    Up,
    Down,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrayItemRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrayItemPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrayItemClick {
    pub id: TrayItemId,
    pub rect: TrayItemRect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawTrayIconEvent {
    pub id: String,
    pub position: TrayItemPoint,
    pub rect: TrayItemRect,
    pub button: TrayMouseButton,
    pub button_state: TrayMouseButtonState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawTrayMenuEvent {
    pub id: String,
}

const MAIN_TRAY_ID: &str = "qingqi.tray.main";
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
            .with_id(MAIN_TRAY_ID)
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

pub fn register_item(spec: TrayItemSpec) -> Result<(), String> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let mut builder = TrayIconBuilder::new()
            .with_id(spec.id.tray_icon_id())
            .with_tooltip(spec.tooltip.as_str())
            .with_menu_on_left_click(false);
        if let Some(icon) = icon_for_tray_item(&spec.icon)? {
            builder = builder.with_icon(icon);
        }

        if !spec.title.is_empty() {
            builder = builder.with_title(spec.title.as_str());
        }

        #[cfg(target_os = "macos")]
        {
            builder = builder.with_icon_as_template(true);
        }

        let tray = builder.build().map_err(|error| error.to_string())?;
        apply_item_title_style(&tray, &spec.title);
        tray.set_visible(spec.visible)
            .map_err(|error| error.to_string())?;
        with_item_trays(|items| {
            items.insert(
                spec.id.clone(),
                TrayItemEntry {
                    state: TrayItemState { spec },
                    tray,
                },
            );
        });
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = spec;
        Err(String::from("system tray not supported on this platform"))
    }
}

pub fn update_item(spec: TrayItemSpec) -> Result<(), String> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let mut found = false;
        let mut result = Ok(());
        with_item_trays(|items| {
            if let Some(entry) = items.get_mut(&spec.id) {
                found = true;
                entry.tray.set_title(Some(spec.title.as_str()));
                apply_item_title_style(&entry.tray, &spec.title);
                if let Err(error) = entry.tray.set_tooltip(Some(spec.tooltip.as_str())) {
                    result = Err(error.to_string());
                    return;
                }
                if let Err(error) = entry.tray.set_visible(spec.visible) {
                    result = Err(error.to_string());
                    return;
                }
                entry.state = TrayItemState { spec: spec.clone() };
            }
        });
        if !found {
            return register_item(spec);
        }
        result
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = spec;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn apply_item_title_style(tray: &tray_icon::TrayIcon, title: &str) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSBaselineOffsetAttributeName, NSFont, NSFontAttributeName, NSFontWeightRegular,
        NSMutableParagraphStyle, NSParagraphStyleAttributeName, NSTextAlignment,
    };
    use objc2_foundation::{NSDictionary, NSMutableAttributedString, NSNumber, NSRange, NSString};

    let Some(status_item) = tray.ns_status_item() else {
        return;
    };
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(button) = status_item.button(mtm) else {
        return;
    };

    if !title.contains('\n') {
        button.setFont(None);
        return;
    }

    let font = NSFont::monospacedSystemFontOfSize_weight(8.8, unsafe { NSFontWeightRegular });
    let paragraph = NSMutableParagraphStyle::new();
    paragraph.setAlignment(NSTextAlignment::Left);
    paragraph.setLineSpacing(0.0);
    paragraph.setMinimumLineHeight(9.6);
    paragraph.setMaximumLineHeight(9.6);

    let baseline = NSNumber::new_f64(-2.2);
    let ns_title = NSString::from_str(title);
    let attributed = NSMutableAttributedString::from_nsstring(&ns_title);
    let range = NSRange::new(0, ns_title.len_utf16());
    let attrs = unsafe {
        NSDictionary::from_slices(
            &[
                NSFontAttributeName,
                NSParagraphStyleAttributeName,
                NSBaselineOffsetAttributeName,
            ],
            &[font.as_ref(), paragraph.as_ref(), baseline.as_ref()],
        )
    };
    unsafe {
        attributed.addAttributes_range(&attrs, range);
    }
    button.setAttributedTitle(&attributed);
}

#[cfg(not(target_os = "macos"))]
fn apply_item_title_style(_tray: &tray_icon::TrayIcon, _title: &str) {}

pub fn remove_item(id: &TrayItemId) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    with_item_trays(|items| {
        items.remove(id);
    });

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = id;
}

pub fn set_item_visible(id: &TrayItemId, visible: bool) -> Result<(), String> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let mut result = Ok(());
        with_item_trays(|items| {
            if let Some(entry) = items.get_mut(id) {
                entry.state.spec.visible = visible;
                if let Err(error) = entry.tray.set_visible(visible) {
                    result = Err(error.to_string());
                }
            }
        });
        result
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = id;
        let _ = visible;
        Ok(())
    }
}

pub fn item_states() -> Vec<TrayItemState> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let mut states = Vec::new();
        with_item_trays(|items| {
            states.extend(items.values().map(|entry| entry.state.clone()));
        });
        states.sort_by(|a, b| {
            a.spec
                .priority
                .cmp(&b.spec.priority)
                .then_with(|| a.spec.id.as_str().cmp(b.spec.id.as_str()))
        });
        states
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}

pub fn next_raw_tray_icon_event() -> Option<RawTrayIconEvent> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        use tray_icon::TrayIconEvent;

        let event = TrayIconEvent::receiver().recv().ok()?;
        match event {
            TrayIconEvent::Click {
                id,
                position,
                rect,
                button,
                button_state,
                ..
            } => Some(RawTrayIconEvent {
                id: id.as_ref().to_string(),
                position: tray_point_from_platform(position),
                rect: tray_rect_from_platform(rect),
                button: tray_mouse_button(button),
                button_state: tray_mouse_button_state(button_state),
            }),
            _ => Some(RawTrayIconEvent {
                id: String::new(),
                position: TrayItemPoint { x: 0.0, y: 0.0 },
                rect: TrayItemRect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
                button: TrayMouseButton::Other,
                button_state: TrayMouseButtonState::Other,
            }),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

pub fn next_raw_menu_event() -> Option<RawTrayMenuEvent> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let event = MenuEvent::receiver().recv().ok()?;
        Some(RawTrayMenuEvent {
            id: event.id().as_ref().to_string(),
        })
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
thread_local! {
    static CURRENT_TRAY: RefCell<Option<TrayIcon>> = const { RefCell::new(None) };
    static ITEM_TRAYS: RefCell<HashMap<TrayItemId, TrayItemEntry>> = RefCell::new(HashMap::new());
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
struct TrayItemEntry {
    state: TrayItemState,
    tray: TrayIcon,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn replace_tray(tray: TrayIcon) {
    CURRENT_TRAY.with(|current| *current.borrow_mut() = Some(tray));
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn with_tray(f: impl FnOnce(&TrayIcon)) {
    CURRENT_TRAY.with(|current| {
        if let Some(tray) = current.borrow().as_ref() {
            f(tray);
        }
    });
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn with_item_trays(f: impl FnOnce(&mut HashMap<TrayItemId, TrayItemEntry>)) {
    ITEM_TRAYS.with(|items| f(&mut items.borrow_mut()));
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
fn icon_for_tray_item(icon: &TrayItemIcon) -> Result<Option<Icon>, String> {
    match icon {
        #[cfg(target_os = "macos")]
        TrayItemIcon::None => Ok(None),
        #[cfg(not(target_os = "macos"))]
        TrayItemIcon::None => default_icon().map(Some),
        TrayItemIcon::Default => default_icon().map(Some),
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn load_tray_svg_icon() -> Option<Icon> {
    // macOS 菜单栏标准逻辑尺寸为 22pt，使用 2x 位图保证 Retina 清晰
    const SIZE: u32 = 44;
    let rgba = crate::svg_icon::rasterize_square(
        include_bytes!("../../qingqi/assets/tray-icon.svg"),
        SIZE,
    )
    .ok()?;
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn tray_point_from_platform(position: tray_icon::dpi::PhysicalPosition<f64>) -> TrayItemPoint {
    TrayItemPoint {
        x: position.x,
        y: position.y,
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn tray_rect_from_platform(rect: tray_icon::Rect) -> TrayItemRect {
    TrayItemRect {
        x: rect.position.x,
        y: rect.position.y,
        width: rect.size.width as f64,
        height: rect.size.height as f64,
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn tray_mouse_button(button: tray_icon::MouseButton) -> TrayMouseButton {
    match button {
        tray_icon::MouseButton::Left => TrayMouseButton::Left,
        tray_icon::MouseButton::Right => TrayMouseButton::Right,
        tray_icon::MouseButton::Middle => TrayMouseButton::Middle,
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn tray_mouse_button_state(state: tray_icon::MouseButtonState) -> TrayMouseButtonState {
    match state {
        tray_icon::MouseButtonState::Up => TrayMouseButtonState::Up,
        tray_icon::MouseButtonState::Down => TrayMouseButtonState::Down,
    }
}

#[cfg(test)]
mod tests {
    use super::{TrayItemIcon, TrayItemId, TrayItemSpec, TrayItemState};

    #[test]
    fn tray_item_id_maps_to_internal_icon_id() {
        let id = TrayItemId::new("network-speed");
        assert_eq!(id.as_str(), "network-speed");
        assert_eq!(id.tray_icon_id(), "qingqi.tray.item.network-speed");
    }

    #[test]
    fn tray_item_states_sort_by_priority_then_id() {
        let mut states = [state("zeta", 20), state("alpha", 10), state("beta", 10)];
        states.sort_by(|a, b| {
            a.spec
                .priority
                .cmp(&b.spec.priority)
                .then_with(|| a.spec.id.as_str().cmp(b.spec.id.as_str()))
        });
        let ids: Vec<&str> = states.iter().map(|state| state.spec.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "beta", "zeta"]);
    }

    fn state(id: &str, priority: i32) -> TrayItemState {
        TrayItemState {
            spec: TrayItemSpec {
                id: TrayItemId::new(id),
                icon: TrayItemIcon::Default,
                title: String::new(),
                tooltip: String::new(),
                menu: Vec::new(),
                priority,
                visible: true,
            },
        }
    }
}

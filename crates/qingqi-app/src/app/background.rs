use std::{
    collections::HashSet,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

use global_hotkey::HotKeyState;
use gpui::{App, BorrowAppContext, Global, Task};

use gpui_component::theme::Theme;

use crate::{
    app::{
        theme_store::ThemeStore,
        tray_manager::{TrayManager, TrayManagerHandle},
        window_controller::{WindowController, WindowControllerHandle},
    },
    core::shortcut::{self, ShortcutGlobal},
};
use qingqi_core::lock_or_recover;
use qingqi_platform::{
    power::{PowerChangeListener, PowerManager},
    theme::ThemeChangeListener,
    tray::{
        RawTrayIconEvent, RawTrayMenuEvent, TrayIconAction, TrayItemClick, TrayItemRect,
        TrayMouseButton, TrayMouseButtonState,
    },
};

#[derive(Default)]
pub struct BackgroundSupervisor {
    running: HashSet<&'static str>,
    tasks: Arc<Mutex<Vec<Task<()>>>>,
    last_main_tray_click: Option<Instant>,
}

impl Global for BackgroundSupervisor {}

impl BackgroundSupervisor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_tray_events(
        &mut self,
        window_controller: WindowControllerHandle,
        tray_manager: TrayManagerHandle,
        power_manager: Arc<Mutex<PowerManager>>,
        cx: &mut App,
    ) {
        if !self.mark_started("tray-events") {
            return;
        }

        arm_tray_icon_event(
            Arc::clone(&self.tasks),
            Arc::clone(&window_controller),
            tray_manager,
            Arc::clone(&power_manager),
            cx,
        );
        arm_tray_menu_event(
            Arc::clone(&self.tasks),
            window_controller,
            power_manager,
            cx,
        );
    }

    pub fn start_hotkey_events(&mut self, window_controller: WindowControllerHandle, cx: &mut App) {
        if !self.mark_started("hotkey-events") {
            return;
        }

        let receiver = qingqi_platform::hotkey::event_receiver();
        let task = cx.spawn(async move |async_cx| {
            loop {
                let receiver = receiver.clone();
                let event = async_cx
                    .background_executor()
                    .spawn(async move { receiver.recv().ok() })
                    .await;
                let Some(event) = event else {
                    break;
                };
                if event.state != HotKeyState::Pressed {
                    continue;
                }
                let window_controller = Arc::clone(&window_controller);
                let _ = async_cx.update(move |cx| {
                    let target = cx
                        .try_global::<ShortcutGlobal>()
                        .and_then(|service| service.dispatch_global(event.id));
                    if let Some(target) = target {
                        shortcut::dispatch_target(&target, window_controller, cx);
                    }
                });
            }
        });
        push_background_task(&self.tasks, task);
    }

    /// Start a low-level keyboard hook (WH_KEYBOARD_LL) for shortcuts that
    /// cannot be registered via `RegisterHotKey` on Windows (e.g. Alt+Space).
    #[cfg(target_os = "windows")]
    pub fn start_low_level_hook(
        &mut self,
        entries: Vec<qingqi_platform::low_level_hook::LowLevelEntry>,
        window_controller: WindowControllerHandle,
        cx: &mut App,
    ) {
        if !self.mark_started("low-level-hook") {
            return;
        }

        if entries.is_empty() {
            tracing::debug!("no low-level hook entries to install");
            return;
        }

        let (hook, rx) = match qingqi_platform::low_level_hook::LowLevelHook::install(entries) {
            Ok(pair) => pair,
            Err(error) => {
                tracing::warn!(error = %error, "failed to install low-level keyboard hook");
                return;
            }
        };

        tracing::debug!("low-level keyboard hook installed");

        let rx = Arc::new(Mutex::new(rx));
        let task = cx.spawn(async move |async_cx| {
            // Keep the hook alive while waiting for events.
            let _hook = hook;
            loop {
                let rx = Arc::clone(&rx);
                let result = async_cx
                    .background_executor()
                    .spawn(async move { rx.lock().ok()?.recv().ok() })
                    .await;
                let Some(hook_id) = result else {
                    break;
                };
                let window_controller = Arc::clone(&window_controller);
                let _ = async_cx.update(move |cx| {
                    let target = cx
                        .try_global::<ShortcutGlobal>()
                        .and_then(|service| service.dispatch_low_level(hook_id));
                    if let Some(target) = target {
                        shortcut::dispatch_target(&target, window_controller, cx);
                    }
                });
            }
        });
        push_background_task(&self.tasks, task);
    }

    /// 通过 `NSDistributedNotificationCenter` 监听系统主题变化，
    /// 替代轮询 `defaults read -g AppleInterfaceStyle`。
    pub fn start_theme_listener(&mut self, theme_store: Arc<RwLock<ThemeStore>>, cx: &mut App) {
        if !self.mark_started("theme-listener") {
            return;
        }

        let listener = ThemeChangeListener::new();
        let rx = listener.receiver();

        let task = cx.spawn(async move |async_cx| {
            // 保持 listener 存活
            let _listener = listener;
            loop {
                let rx = Arc::clone(&rx);
                let recv_result = async_cx
                    .background_executor()
                    .spawn(async move { rx.lock().ok()?.recv().ok() })
                    .await;
                if recv_result.is_none() {
                    break;
                }
                let store = Arc::clone(&theme_store);
                let _ = async_cx.update(move |cx| {
                    if let Ok(mut ts) = store.write() {
                        let _ = ts.sync_system_changed();
                    }
                    Theme::sync_system_appearance(None, cx);
                });
            }
        });
        push_background_task(&self.tasks, task);
    }

    /// 通过 `IOPSNotificationCreateRunLoopSource` 监听电源变化，替代轮询。
    pub fn start_power_listener(&mut self, power_manager: Arc<Mutex<PowerManager>>, cx: &mut App) {
        if !self.mark_started("power-listener") {
            return;
        }

        let listener = PowerChangeListener::new();
        let rx = listener.receiver();

        let task = cx.spawn(async move |async_cx| {
            let _listener = listener;
            loop {
                let rx = Arc::clone(&rx);
                let recv_result = async_cx
                    .background_executor()
                    .spawn(async move { rx.lock().ok()?.recv().ok() })
                    .await;
                if recv_result.is_none() {
                    break;
                }
                let pm = Arc::clone(&power_manager);
                let _ = async_cx.update(move |_cx| {
                    lock_or_recover(&pm, "power-manager").update();
                });
            }
        });
        push_background_task(&self.tasks, task);
    }

    fn mark_started(&mut self, name: &'static str) -> bool {
        if self.running.insert(name) {
            true
        } else {
            tracing::debug!(task = name, "background task already running");
            false
        }
    }
}

fn push_background_task(tasks: &Arc<Mutex<Vec<Task<()>>>>, task: Task<()>) {
    if let Ok(mut tasks) = tasks.lock() {
        tasks.push(task);
    } else {
        task.detach();
    }
}

fn arm_tray_icon_event(
    tasks: Arc<Mutex<Vec<Task<()>>>>,
    window_controller: WindowControllerHandle,
    tray_manager: TrayManagerHandle,
    power_manager: Arc<Mutex<PowerManager>>,
    cx: &mut App,
) {
    let tasks_for_task = Arc::clone(&tasks);
    let task = cx.spawn(async move |async_cx| {
        let event = async_cx
            .background_executor()
            .spawn(async { qingqi_platform::tray::next_raw_tray_icon_event() })
            .await;
        let Some(event) = event else {
            return;
        };
        let wc = Arc::clone(&window_controller);
        let tm = tray_manager.clone();
        let pm = Arc::clone(&power_manager);
        let tasks_for_rearm = Arc::clone(&tasks_for_task);
        let _ = async_cx.update(move |cx| {
            if let Some(action) = action_for_tray_icon_event(event) {
                if matches!(action, TrayIconAction::Main) && is_duplicate_main_tray_click(cx) {
                    arm_tray_icon_event(tasks_for_rearm, wc, tm, pm, cx);
                    return;
                }
                handle_tray_icon_action(action, Arc::clone(&wc), tm.clone(), Arc::clone(&pm), cx);
            }
            arm_tray_icon_event(tasks_for_rearm, wc, tm, pm, cx);
        });
    });
    push_background_task(&tasks, task);
}

fn arm_tray_menu_event(
    tasks: Arc<Mutex<Vec<Task<()>>>>,
    window_controller: WindowControllerHandle,
    power_manager: Arc<Mutex<PowerManager>>,
    cx: &mut App,
) {
    let tasks_for_task = Arc::clone(&tasks);
    let task = cx.spawn(async move |async_cx| {
        let event = async_cx
            .background_executor()
            .spawn(async { qingqi_platform::tray::next_raw_menu_event() })
            .await;
        let Some(event) = event else {
            return;
        };
        let wc = Arc::clone(&window_controller);
        let pm = Arc::clone(&power_manager);
        let tasks_for_rearm = Arc::clone(&tasks_for_task);
        let _ = async_cx.update(move |cx| {
            if let Some(action) = action_for_menu_event(event) {
                handle_tray_action(action, Arc::clone(&wc), Arc::clone(&pm), cx);
            }
            arm_tray_menu_event(tasks_for_rearm, wc, pm, cx);
        });
    });
    push_background_task(&tasks, task);
}

fn action_for_tray_icon_event(event: RawTrayIconEvent) -> Option<TrayIconAction> {
    if event.button != TrayMouseButton::Left || event.button_state != TrayMouseButtonState::Up {
        return None;
    }
    if event.id == "qingqi.tray.main" {
        return Some(TrayIconAction::Main);
    }
    event.id.strip_prefix("qingqi.tray.item.").map(|id| {
        TrayIconAction::Item(TrayItemClick {
            id: qingqi_platform::tray::TrayItemId::new(id),
            rect: rect_for_tray_icon_event(&event),
        })
    })
}

fn rect_for_tray_icon_event(event: &RawTrayIconEvent) -> TrayItemRect {
    let width = event.rect.width.max(22.0);
    let height = event.rect.height.max(22.0);
    if event.position.x.abs() <= 1.0 && event.position.y.abs() <= 1.0 {
        return event.rect;
    }

    TrayItemRect {
        x: event.position.x - width / 2.0,
        y: event.position.y - height / 2.0,
        width,
        height,
    }
}

fn is_duplicate_main_tray_click(cx: &mut App) -> bool {
    const DUPLICATE_CLICK_WINDOW: Duration = Duration::from_millis(180);
    let now = Instant::now();
    cx.update_global::<BackgroundSupervisor, bool>(|background, _cx| {
        let duplicate = background
            .last_main_tray_click
            .is_some_and(|last| now.saturating_duration_since(last) <= DUPLICATE_CLICK_WINDOW);
        background.last_main_tray_click = Some(now);
        duplicate
    })
}

fn action_for_menu_event(event: RawTrayMenuEvent) -> Option<qingqi_platform::tray::TrayAction> {
    use qingqi_platform::{power::PreventSleepMode, tray::TrayAction};

    match event.id.as_str() {
        "qingqi.tray.show" => Some(TrayAction::Show),
        "qingqi.tray.sleep.disabled" => {
            Some(TrayAction::SetPreventSleep(PreventSleepMode::Disabled))
        }
        "qingqi.tray.sleep.always" => Some(TrayAction::SetPreventSleep(PreventSleepMode::AlwaysOn)),
        "qingqi.tray.sleep.plugged" => {
            Some(TrayAction::SetPreventSleep(PreventSleepMode::WhenPluggedIn))
        }
        "qingqi.tray.restart" => Some(TrayAction::Restart),
        "qingqi.tray.quit" => Some(TrayAction::Quit),
        _ => None,
    }
}

fn handle_tray_action(
    action: qingqi_platform::tray::TrayAction,
    window_controller: WindowControllerHandle,
    power_manager: Arc<Mutex<PowerManager>>,
    cx: &mut App,
) {
    use qingqi_platform::tray::TrayAction;

    match action {
        TrayAction::Show => {
            WindowController::show_launcher(window_controller, cx);
        }
        TrayAction::SetPreventSleep(mode) => {
            lock_or_recover(&power_manager, "power-manager").set_mode(mode);
            let _ = qingqi_platform::tray::rebuild_menu(mode);
        }
        TrayAction::Restart => {
            qingqi_platform::tray::relaunch();
            cx.quit();
        }
        TrayAction::Quit => cx.quit(),
    }
}

fn handle_tray_icon_action(
    action: qingqi_platform::tray::TrayIconAction,
    window_controller: WindowControllerHandle,
    tray_manager: TrayManagerHandle,
    power_manager: Arc<Mutex<PowerManager>>,
    cx: &mut App,
) {
    match action {
        TrayIconAction::Main => {
            handle_tray_action(
                qingqi_platform::tray::TrayAction::Show,
                window_controller,
                power_manager,
                cx,
            );
        }
        TrayIconAction::Item(click) => {
            TrayManager::handle_item_click(tray_manager, click, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{action_for_tray_icon_event, rect_for_tray_icon_event};
    use qingqi_platform::tray::{
        RawTrayIconEvent, TrayIconAction, TrayItemRect, TrayMouseButton, TrayMouseButtonState,
    };

    #[test]
    fn tray_main_click_only_handles_left_button_release() {
        let action = action_for_tray_icon_event(raw_event(
            "qingqi.tray.main",
            TrayMouseButton::Left,
            TrayMouseButtonState::Up,
        ));

        assert!(matches!(action, Some(TrayIconAction::Main)));
    }

    #[test]
    fn tray_click_ignores_button_press_to_avoid_toggle_flash() {
        let action = action_for_tray_icon_event(raw_event(
            "qingqi.tray.main",
            TrayMouseButton::Left,
            TrayMouseButtonState::Down,
        ));

        assert!(action.is_none());
    }

    #[test]
    fn tray_item_rect_anchors_to_cursor_position() {
        let event = raw_event(
            "qingqi.tray.item.network-speed",
            TrayMouseButton::Left,
            TrayMouseButtonState::Up,
        );
        let rect = rect_for_tray_icon_event(&event);

        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 0.0);
        assert_eq!(rect.width, 22.0);
        assert_eq!(rect.height, 22.0);
    }

    fn raw_event(
        id: &str,
        button: TrayMouseButton,
        button_state: TrayMouseButtonState,
    ) -> RawTrayIconEvent {
        RawTrayIconEvent {
            id: id.to_string(),
            position: qingqi_platform::tray::TrayItemPoint { x: 11.0, y: 11.0 },
            rect: TrayItemRect {
                x: 0.0,
                y: 0.0,
                width: 22.0,
                height: 22.0,
            },
            button,
            button_state,
        }
    }
}

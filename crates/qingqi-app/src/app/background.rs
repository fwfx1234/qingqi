use std::{
    collections::HashSet,
    sync::{Arc, Mutex, RwLock},
};

use global_hotkey::HotKeyState;
use gpui::{App, Global, Task};

use gpui_component::theme::Theme;

use crate::{
    app::{
        theme_store::ThemeStore,
        window_controller::{WindowController, WindowControllerHandle},
    },
    core::shortcut::{self, ShortcutGlobal},
};
use qingqi_core::lock_or_recover;
use qingqi_platform::{
    power::{PowerChangeListener, PowerManager},
    theme::ThemeChangeListener,
};

#[derive(Default)]
pub struct BackgroundSupervisor {
    running: HashSet<&'static str>,
    tasks: Vec<Task<()>>,
}

impl Global for BackgroundSupervisor {}

impl BackgroundSupervisor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_tray_events(
        &mut self,
        window_controller: WindowControllerHandle,
        power_manager: Arc<Mutex<PowerManager>>,
        cx: &mut App,
    ) {
        if !self.mark_started("tray-events") {
            return;
        }

        // Tray-icon left-clicks → show launcher. Event-driven: the helper parks
        // on the tray channel, so there is no idle polling wakeup.
        let icon_wc = Arc::clone(&window_controller);
        let icon_pm = Arc::clone(&power_manager);
        let icon_task = cx.spawn(async move |async_cx| {
            loop {
                let action = async_cx
                    .background_executor()
                    .spawn(async { qingqi_platform::tray::next_tray_action() })
                    .await;
                let Some(action) = action else {
                    break;
                };
                let wc = Arc::clone(&icon_wc);
                let pm = Arc::clone(&icon_pm);
                let _ = async_cx.update(move |cx| handle_tray_action(action, wc, pm, cx));
            }
        });
        self.tasks.push(icon_task);

        // Tray menu selections → their mapped actions.
        let menu_task = cx.spawn(async move |async_cx| {
            loop {
                let action = async_cx
                    .background_executor()
                    .spawn(async { qingqi_platform::tray::next_menu_action() })
                    .await;
                let Some(action) = action else {
                    break;
                };
                let wc = Arc::clone(&window_controller);
                let pm = Arc::clone(&power_manager);
                let _ = async_cx.update(move |cx| handle_tray_action(action, wc, pm, cx));
            }
        });
        self.tasks.push(menu_task);
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
        self.tasks.push(task);
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
        self.tasks.push(task);
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
        self.tasks.push(task);
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
        self.tasks.push(task);
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

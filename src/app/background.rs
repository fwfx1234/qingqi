use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use global_hotkey::HotKeyState;
use gpui::{App, Global, Task};

use crate::{
    app::{
        theme_store::ThemeStore,
        window_controller::{WindowController, WindowControllerHandle},
    },
    core::lock_or_recover,
    platform::power::PowerManager,
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

    pub fn start_tray_poll(
        &mut self,
        window_controller: WindowControllerHandle,
        power_manager: Arc<Mutex<PowerManager>>,
        cx: &mut App,
    ) {
        if !self.mark_started("tray-poll") {
            return;
        }

        let task = cx.spawn(async move |async_cx| {
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                let actions = crate::platform::tray::poll_actions();
                if actions.is_empty() {
                    continue;
                }
                let window_controller = Arc::clone(&window_controller);
                let pm = Arc::clone(&power_manager);
                let _ = async_cx.update(move |cx| {
                    for action in actions {
                        handle_tray_action(
                            action,
                            Arc::clone(&window_controller),
                            Arc::clone(&pm),
                            cx,
                        );
                    }
                });
            }
        });
        self.tasks.push(task);
    }

    pub fn start_hotkey_events(&mut self, window_controller: WindowControllerHandle, cx: &mut App) {
        if !self.mark_started("hotkey-events") {
            return;
        }

        let receiver = crate::platform::hotkey::event_receiver();
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
                        .try_global::<crate::core::shortcut::ShortcutService>()
                        .and_then(|service| service.dispatch_global(event.id));
                    if let Some(target) = target {
                        crate::core::shortcut::dispatch_target(&target, window_controller, cx);
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
        window_controller: WindowControllerHandle,
        cx: &mut App,
    ) {
        if !self.mark_started("low-level-hook") {
            return;
        }

        let shortcut_service = cx.try_global::<crate::core::shortcut::ShortcutService>();
        let Some(service) = shortcut_service else {
            tracing::warn!("ShortcutService not available; skipping low-level hook");
            return;
        };

        let entries = service.low_level_entries().to_vec();
        if entries.is_empty() {
            tracing::debug!("no low-level hook entries to install");
            return;
        }

        let (hook, rx) = match crate::platform::low_level_hook::LowLevelHook::install(entries) {
            Ok(pair) => pair,
            Err(error) => {
                tracing::warn!(error = %error, "failed to install low-level keyboard hook");
                return;
            }
        };

        tracing::debug!("low-level keyboard hook installed");

        let task = cx.spawn(async move |async_cx| {
            // Keep the hook alive while polling for events.
            let _hook = hook;
            loop {
                // Poll rx periodically — WH_KEYBOARD_LL events arrive at
                // human timescales, so 50 ms is more than responsive enough.
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                while let Ok(hook_id) = rx.try_recv() {
                    let window_controller = Arc::clone(&window_controller);
                    let _ = async_cx.update(move |cx| {
                        let target = cx
                            .try_global::<crate::core::shortcut::ShortcutService>()
                            .and_then(|service| service.dispatch_low_level(hook_id));
                        if let Some(target) = target {
                            crate::core::shortcut::dispatch_target(
                                &target,
                                window_controller,
                                cx,
                            );
                        }
                    });
                }
            }
        });
        self.tasks.push(task);
    }

    pub fn start_theme_poll(&mut self, theme_store: Arc<Mutex<ThemeStore>>, cx: &mut App) {
        if !self.mark_started("theme-poll") {
            return;
        }

        let task = cx.spawn(async move |async_cx| {
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_secs(3))
                    .await;
                let store = Arc::clone(&theme_store);
                let _ = async_cx.update(move |_cx| {
                    if let Ok(mut ts) = store.lock() {
                        let _ = ts.sync_system_changed();
                    }
                });
            }
        });
        self.tasks.push(task);
    }

    /// Poll power state every 5 s to handle WhenPluggedIn mode transitions.
    pub fn start_power_poll(&mut self, power_manager: Arc<Mutex<PowerManager>>, cx: &mut App) {
        if !self.mark_started("power-poll") {
            return;
        }

        let task = cx.spawn(async move |async_cx| {
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_secs(5))
                    .await;
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
    action: crate::platform::tray::TrayAction,
    window_controller: WindowControllerHandle,
    power_manager: Arc<Mutex<PowerManager>>,
    cx: &mut App,
) {
    use crate::platform::tray::TrayAction;

    match action {
        TrayAction::Show => {
            WindowController::show_launcher(window_controller, cx);
        }
        TrayAction::SetPreventSleep(mode) => {
            lock_or_recover(&power_manager, "power-manager").set_mode(mode);
            let _ = crate::platform::tray::rebuild_menu(mode);
        }
        TrayAction::Restart => {
            crate::platform::tray::relaunch();
            cx.quit();
        }
        TrayAction::Quit => cx.quit(),
    }
}

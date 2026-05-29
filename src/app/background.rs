use std::{
    cell::RefCell,
    collections::HashSet,
    rc::Rc,
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
        power_manager: Rc<RefCell<PowerManager>>,
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
                let window_controller = Rc::clone(&window_controller);
                let pm = Rc::clone(&power_manager);
                let _ = async_cx.update(move |cx| {
                    for action in actions {
                        handle_tray_action(
                            action,
                            Rc::clone(&window_controller),
                            Rc::clone(&pm),
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
                let window_controller = Rc::clone(&window_controller);
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
    pub fn start_power_poll(&mut self, power_manager: Rc<RefCell<PowerManager>>, cx: &mut App) {
        if !self.mark_started("power-poll") {
            return;
        }

        let task = cx.spawn(async move |async_cx| {
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_secs(5))
                    .await;
                let pm = Rc::clone(&power_manager);
                let _ = async_cx.update(move |_cx| {
                    pm.borrow_mut().update();
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
    power_manager: Rc<RefCell<PowerManager>>,
    cx: &mut App,
) {
    use crate::platform::tray::TrayAction;

    match action {
        TrayAction::Show => {
            WindowController::show_launcher(window_controller, cx);
        }
        TrayAction::SetPreventSleep(mode) => {
            power_manager.borrow_mut().set_mode(mode);
            let _ = crate::platform::tray::rebuild_menu(mode);
        }
        TrayAction::Restart => {
            crate::platform::tray::relaunch();
            cx.quit();
        }
        TrayAction::Quit => cx.quit(),
    }
}

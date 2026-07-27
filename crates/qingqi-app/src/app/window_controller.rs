use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use gpui::{
    AnyWindowHandle, App, Bounds, Context, Focusable, IntoElement, ParentElement, Render, Styled,
    TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowDecorations,
    WindowKind, WindowOptions, div, prelude::*, px, size,
};
use qingqi_ui::components::root::Root;

use crate::app::{
    app_catalog::AppCatalog,
    dock_agent::{DockAgentConfig, DockAgentManager},
    launcher::Launcher,
};
use qingqi_core::lock_or_recover;
use qingqi_core::plugin::{PluginManager, WindowView};
use qingqi_plugin::command::{Action, Activation, CommandInvocation};
use qingqi_plugin::events::AppEventBus;
use qingqi_plugin::plugin_spec::{WindowBackgroundSpec, WindowSize};
use qingqi_ui::components::input::InputState;
use qingqi_ui::ui;

pub type WindowControllerHandle = Arc<Mutex<WindowController>>;

#[derive(Clone, Copy, Debug)]
pub struct PluginOpenTrace {
    pub id: u64,
    pub started: Instant,
}

impl PluginOpenTrace {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            started: Instant::now(),
        }
    }
}

pub struct WindowController {
    plugin_manager: Arc<Mutex<PluginManager>>,
    app_catalog: Arc<AppCatalog>,
    events: AppEventBus,
    launcher_window: Option<AnyWindowHandle>,
    plugin_windows: HashMap<String, AnyWindowHandle>,
    dock_agents: DockAgentManager,
    #[cfg(target_os = "windows")]
    keep_alive_window: Option<AnyWindowHandle>,
}

impl WindowController {
    pub fn new(
        plugin_manager: Arc<Mutex<PluginManager>>,
        app_catalog: Arc<AppCatalog>,
        events: AppEventBus,
    ) -> Self {
        Self {
            plugin_manager,
            app_catalog,
            events,
            launcher_window: None,
            plugin_windows: HashMap::new(),
            dock_agents: DockAgentManager::default(),
            #[cfg(target_os = "windows")]
            keep_alive_window: None,
        }
    }

    pub fn plugin_manager(&self) -> Arc<Mutex<PluginManager>> {
        Arc::clone(&self.plugin_manager)
    }

    pub fn app_catalog(&self) -> Arc<AppCatalog> {
        Arc::clone(&self.app_catalog)
    }

    #[cfg(target_os = "windows")]
    pub fn ensure_keep_alive_window(&mut self, cx: &mut App) {
        if self.keep_alive_window.is_some() {
            return;
        }

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                gpui::point(px(-10000.0), px(-10000.0)),
                size(px(1.0), px(1.0)),
            ))),
            titlebar: None,
            focus: false,
            show: false,
            kind: WindowKind::Normal,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            window_decorations: Some(WindowDecorations::Client),
            ..Default::default()
        };

        match cx.open_window(options, |_window, cx| cx.new(|_| KeepAliveWindow)) {
            Ok(handle) => {
                self.keep_alive_window = Some(handle.into());
            }
            Err(error) => tracing::warn!(error = %error, "open keep-alive window failed"),
        }
    }

    pub fn toggle_launcher(controller: WindowControllerHandle, cx: &mut App) {
        let stored_window_handle =
            { lock_or_recover(&controller, "window_controller").launcher_window };
        if let Some(window_handle) = stored_window_handle {
            match update_launcher(window_handle, cx, |launcher, window, cx| {
                launcher.cleanup_before_close(window, cx);
                window.defer(cx, |window, _cx| window.remove_window());
            }) {
                Ok(_) => {
                    lock_or_recover(&controller, "window_controller").launcher_window = None;
                    return;
                }
                Err(error) => {
                    tracing::warn!(error = %error, "toggle existing launcher window failed");
                    lock_or_recover(&controller, "window_controller").launcher_window = None;
                }
            }
        }

        Self::show_launcher(controller, cx);
    }

    pub fn show_launcher(controller: WindowControllerHandle, cx: &mut App) {
        let stored_window_handle =
            { lock_or_recover(&controller, "window_controller").launcher_window };
        if let Some(window_handle) = stored_window_handle {
            match update_launcher(window_handle, cx, |launcher, window, cx| {
                qingqi_platform::macos::activate_frontmost();
                cx.activate(true);
                window.activate_window();
                launcher.refresh_on_show(cx);
            }) {
                Ok(_) => {
                    // 在独立的 update 中设置焦点，确保 refresh_on_show
                    // 触发的重渲染完成后再请求焦点
                    let _ = update_launcher(window_handle, cx, |launcher, window, cx| {
                        activate_launcher_window(launcher, window, cx);
                    });
                    return;
                }
                Err(error) => {
                    tracing::warn!(error = %error, "activate existing launcher window failed");
                    lock_or_recover(&controller, "window_controller").launcher_window = None;
                }
            }
        }

        Self::open_launcher(controller, cx);
    }

    fn open_launcher(controller: WindowControllerHandle, cx: &mut App) {
        let plugin_manager = lock_or_recover(&controller, "window_controller").plugin_manager();
        let app_catalog = lock_or_recover(&controller, "window_controller").app_catalog();
        let events = lock_or_recover(&controller, "window_controller")
            .events
            .clone();
        let window_size = size(
            px(Launcher::window_width()),
            px(Launcher::default_window_height()),
        );
        let window_min_size = size(
            px(Launcher::window_width()),
            px(Launcher::min_window_height()),
        );
        let (display, bounds) =
            qingqi_platform::display::centered_on_active_display(cx, window_size);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            display_id: display.map(|display| display.id()),
            titlebar: Some(TitlebarOptions {
                title: Some("启程 (Qingqi)".into()),
                appears_transparent: true,
                traffic_light_position: Some(gpui::point(px(-80.0), px(-80.0))),
                ..Default::default()
            }),
            kind: WindowKind::PopUp,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            window_min_size: Some(window_min_size),
            window_decorations: Some(WindowDecorations::Client),
            ..Default::default()
        };
        let controller_for_entity = Arc::clone(&controller);
        match cx.open_window(options, move |window, cx| {
            window.set_window_title("启程 (Qingqi)");
            let query_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("搜索工具、命令、文件..."));
            let launcher = cx
                .new(|cx| Launcher::new(Arc::clone(&plugin_manager), Arc::clone(&app_catalog), cx));
            let handle = launcher.clone();
            launcher.update(cx, |launcher, launcher_cx| {
                launcher.attach_handle(handle.clone());
                launcher.attach_window_controller(Arc::clone(&controller_for_entity));
                launcher.attach_query_input(query_input.clone());
                launcher.bind_query_input_keys(handle, launcher_cx);
                launcher.observe_query_input(launcher_cx);
                launcher.initialize_async(events, launcher_cx);
            });
            // Defer focus to next frame so that the on_focus listener registered
            // via `cx.on_focus` (which uses deferred activate) is active by then.
            let fh = query_input.focus_handle(cx);
            window.on_next_frame(move |window, _cx| {
                window.focus(&fh);
            });
            cx.new(|cx| Root::new(launcher, window, cx))
        }) {
            Ok(handle) => {
                lock_or_recover(&controller, "window_controller").launcher_window =
                    Some(handle.into());
                let _ = update_launcher(handle.into(), cx, |launcher, window, cx| {
                    activate_launcher_window(launcher, window, cx);
                });
            }
            Err(error) => tracing::warn!(error = %error, "open launcher window failed"),
        }
    }

    pub fn open_plugin(
        controller: WindowControllerHandle,
        plugin_id: impl AsRef<str>,
        cx: &mut App,
    ) {
        Self::open_plugin_with_trace(controller, plugin_id, cx, None);
    }

    pub fn open_plugin_with_trace(
        controller: WindowControllerHandle,
        plugin_id: impl AsRef<str>,
        cx: &mut App,
        trace: Option<PluginOpenTrace>,
    ) {
        let plugin_id = plugin_id.as_ref().to_string();
        let started = Instant::now();
        let plugin_manager = lock_or_recover(&controller, "window_controller").plugin_manager();
        let manifest = lock_or_recover(&plugin_manager, "window_controller")
            .manifests()
            .into_iter()
            .find(|manifest| manifest.id.as_ref() == plugin_id);

        if plugin_reopens_in_active_space(manifest.as_ref()) {
            let close_started = Instant::now();
            Self::close_existing_plugin_window(Arc::clone(&controller), &plugin_id, cx);
            log_plugin_window_step(
                &plugin_id,
                "close existing plugin window",
                close_started,
                trace,
            );
        } else if Self::activate_existing_plugin(Arc::clone(&controller), &plugin_id, cx) {
            log_plugin_window_step(&plugin_id, "activate existing plugin", started, trace);
            log_plugin_open_total(
                &plugin_id,
                trace.unwrap_or(PluginOpenTrace { id: 0, started }),
            );
            return;
        }

        let view_started = Instant::now();
        let view = match lock_or_recover(&plugin_manager, "window_controller")
            .open_window_view(&plugin_id, cx)
        {
            Ok(view) => view,
            Err(error) => {
                tracing::warn!(
                    plugin_id,
                    trace_id = trace.map(|trace| trace.id),
                    error = %error,
                    "open plugin failed"
                );
                return;
            }
        };
        log_plugin_window_step(&plugin_id, "open plugin view", view_started, trace);

        let title = view.title().to_string();
        let (display, bounds) = plugin_bounds(manifest.as_ref(), cx);
        let options = plugin_window_options(&title, manifest.as_ref(), display, bounds);
        let client_drawn = manifest.as_ref().is_some_and(|m| {
            m.window.always_on_top || m.window.background == WindowBackgroundSpec::Blurred
        });
        let plugin_id_for_window = plugin_id.clone();
        let controller_for_window = Arc::clone(&controller);
        let window_started = Instant::now();
        match cx.open_window(options, move |window, cx| {
            window.set_window_title(&title);
            let plugin = cx.new(|cx| {
                PluginWindow::new(cx, Arc::clone(&controller_for_window), view, client_drawn)
            });
            cx.new(|cx| {
                let mut root = Root::new(plugin, window, cx);
                if client_drawn {
                    root.set_background(gpui::transparent_black());
                }
                root
            })
        }) {
            Ok(handle) => {
                log_plugin_window_step(&plugin_id, "open plugin window", window_started, trace);
                lock_or_recover(&controller, "window_controller")
                    .set_plugin_window(plugin_id_for_window, handle.into());
                if let Some(config) = dock_agent_config(manifest.as_ref()) {
                    let _ = update_plugin_window(handle.into(), cx, |plugin, _window, cx| {
                        plugin.attach_dock_agent(config, cx);
                    });
                }
                let _ = handle.update(cx, |_, window, cx| {
                    qingqi_platform::macos::activate_frontmost();
                    cx.activate(true);
                    window.activate_window();
                });
            }
            Err(error) => tracing::warn!(
                plugin_id,
                trace_id = trace.map(|trace| trace.id),
                error = %error,
                "open plugin window failed"
            ),
        }
        log_plugin_window_step(&plugin_id, "open plugin local total", started, trace);
        log_plugin_open_total(
            &plugin_id,
            trace.unwrap_or(PluginOpenTrace { id: 0, started }),
        );
    }

    fn activate_existing_plugin(
        controller: WindowControllerHandle,
        plugin_id: &str,
        cx: &mut App,
    ) -> bool {
        let stored_window_handle = {
            lock_or_recover(&controller, "window_controller")
                .plugin_windows
                .get(plugin_id)
                .copied()
        };
        if let Some(window_handle) = stored_window_handle {
            match update_plugin_window(window_handle, cx, |plugin_window, window, cx| {
                qingqi_platform::macos::activate_frontmost();
                cx.activate(true);
                plugin_window.reopen(window, cx);
                qingqi_platform::macos::restore_window(window);
                window.activate_window();
            }) {
                Ok(_) => {
                    cx.activate(true);
                    return true;
                }
                Err(error) => {
                    tracing::warn!(
                        plugin_id,
                        error = %error,
                        "activate existing plugin window failed"
                    );
                    lock_or_recover(&controller, "window_controller")
                        .clear_plugin_window(plugin_id);
                }
            }
        }

        for window_handle in cx.windows() {
            if plugin_id_from_handle(window_handle, cx).as_deref() != Some(plugin_id) {
                continue;
            }

            let _ = update_plugin_window(window_handle, cx, |plugin_window, window, cx| {
                qingqi_platform::macos::activate_frontmost();
                cx.activate(true);
                plugin_window.reopen(window, cx);
                qingqi_platform::macos::restore_window(window);
                window.activate_window();
            });
            lock_or_recover(&controller, "window_controller")
                .set_plugin_window(plugin_id.to_string(), window_handle);
            cx.activate(true);
            return true;
        }

        false
    }

    fn close_existing_plugin_window(
        controller: WindowControllerHandle,
        plugin_id: &str,
        cx: &mut App,
    ) -> bool {
        let stored_window_handle = {
            lock_or_recover(&controller, "window_controller")
                .plugin_windows
                .get(plugin_id)
                .copied()
        };
        if let Some(window_handle) = stored_window_handle {
            match update_plugin_window(window_handle, cx, |_, window, cx| {
                window.defer(cx, |window, _cx| window.remove_window());
            }) {
                Ok(_) => {
                    lock_or_recover(&controller, "window_controller")
                        .clear_plugin_window(plugin_id);
                    return true;
                }
                Err(error) => {
                    tracing::warn!(
                        plugin_id,
                        error = %error,
                        "close existing plugin window failed"
                    );
                    lock_or_recover(&controller, "window_controller")
                        .clear_plugin_window(plugin_id);
                }
            }
        }

        for window_handle in cx.windows() {
            if plugin_id_from_handle(window_handle, cx).as_deref() != Some(plugin_id) {
                continue;
            }

            let closed = update_plugin_window(window_handle, cx, |_, window, cx| {
                window.defer(cx, |window, _cx| window.remove_window());
            })
            .is_ok();
            lock_or_recover(&controller, "window_controller").clear_plugin_window(plugin_id);
            if closed {
                return true;
            }
        }

        false
    }

    pub fn run_command(
        controller: WindowControllerHandle,
        activation: Activation,
        cx: &mut App,
    ) -> Option<String> {
        Self::run_command_with_input_with_trace(controller, activation, cx, None, None)
    }

    pub fn run_command_with_trace(
        controller: WindowControllerHandle,
        activation: Activation,
        cx: &mut App,
        trace: Option<PluginOpenTrace>,
    ) -> Option<String> {
        Self::run_command_with_input_with_trace(controller, activation, cx, trace, None)
    }

    pub fn run_command_with_input(
        controller: WindowControllerHandle,
        activation: Activation,
        cx: &mut App,
        launch_input: Option<String>,
    ) -> Option<String> {
        Self::run_command_with_input_with_trace(controller, activation, cx, None, launch_input)
    }

    pub fn run_command_with_input_with_trace(
        controller: WindowControllerHandle,
        activation: Activation,
        cx: &mut App,
        trace: Option<PluginOpenTrace>,
        launch_input: Option<String>,
    ) -> Option<String> {
        match activation {
            Activation::Open { plugin_id } => {
                Self::open_plugin_with_input_with_trace(
                    controller,
                    plugin_id,
                    cx,
                    trace,
                    launch_input,
                );
                None
            }
            Activation::Run(Action::LaunchApp { path }) => {
                let app_catalog = lock_or_recover(&controller, "window_controller").app_catalog();
                Some(match app_catalog.launch(&path) {
                    Ok(()) => format!("已打开 {}", std::path::Path::new(&path).display()),
                    Err(error) => error,
                })
            }
            activation @ Activation::Run(Action::PluginAction { .. }) => {
                let plugin_id = activation.plugin_id().to_string();
                let plugin_manager =
                    lock_or_recover(&controller, "window_controller").plugin_manager();
                match lock_or_recover(&plugin_manager, "window_controller")
                    .handle_command(CommandInvocation { activation }, cx)
                {
                    Ok(outcome) => outcome.message,
                    Err(error) => {
                        tracing::error!(
                            plugin_id = %plugin_id,
                            error = %error,
                            "command execution failed"
                        );
                        Some(format!("执行失败: {error}"))
                    }
                }
            }
        }
    }

    fn open_plugin_with_input_with_trace(
        controller: WindowControllerHandle,
        plugin_id: impl AsRef<str>,
        cx: &mut App,
        trace: Option<PluginOpenTrace>,
        launch_input: Option<String>,
    ) {
        let plugin_id = plugin_id.as_ref().to_string();
        let started = Instant::now();
        let plugin_manager = lock_or_recover(&controller, "window_controller").plugin_manager();
        let manifest = lock_or_recover(&plugin_manager, "window_controller")
            .manifests()
            .into_iter()
            .find(|manifest| manifest.id.as_ref() == plugin_id);

        if plugin_reopens_in_active_space(manifest.as_ref()) {
            let close_started = Instant::now();
            Self::close_existing_plugin_window(Arc::clone(&controller), &plugin_id, cx);
            log_plugin_window_step(
                &plugin_id,
                "close existing plugin window",
                close_started,
                trace,
            );
        } else if Self::activate_existing_plugin_with_input(
            Arc::clone(&controller),
            &plugin_id,
            cx,
            launch_input.as_deref(),
        ) {
            log_plugin_window_step(&plugin_id, "activate existing plugin", started, trace);
            log_plugin_open_total(
                &plugin_id,
                trace.unwrap_or(PluginOpenTrace { id: 0, started }),
            );
            return;
        }

        let view_started = Instant::now();
        let view = match lock_or_recover(&plugin_manager, "window_controller")
            .open_window_view(&plugin_id, cx)
        {
            Ok(mut view) => {
                if let Some(input) = launch_input.as_deref()
                    && !input.trim().is_empty()
                {
                    view.on_input_changed(input, cx);
                }
                view
            }
            Err(error) => {
                tracing::warn!(
                    plugin_id,
                    trace_id = trace.map(|trace| trace.id),
                    error = %error,
                    "open plugin failed"
                );
                return;
            }
        };
        log_plugin_window_step(&plugin_id, "open plugin view", view_started, trace);

        let title = view.title().to_string();
        let (display, bounds) = plugin_bounds(manifest.as_ref(), cx);
        let options = plugin_window_options(&title, manifest.as_ref(), display, bounds);
        let client_drawn = manifest.as_ref().is_some_and(|m| {
            m.window.always_on_top || m.window.background == WindowBackgroundSpec::Blurred
        });
        let plugin_id_for_window = plugin_id.clone();
        let controller_for_window = Arc::clone(&controller);
        let window_started = Instant::now();
        match cx.open_window(options, move |window, cx| {
            window.set_window_title(&title);
            let plugin = cx.new(|cx| {
                PluginWindow::new(cx, Arc::clone(&controller_for_window), view, client_drawn)
            });
            cx.new(|cx| {
                let mut root = Root::new(plugin, window, cx);
                if client_drawn {
                    root.set_background(gpui::transparent_black());
                }
                root
            })
        }) {
            Ok(handle) => {
                log_plugin_window_step(&plugin_id, "open plugin window", window_started, trace);
                lock_or_recover(&controller, "window_controller")
                    .set_plugin_window(plugin_id_for_window, handle.into());
                if let Some(config) = dock_agent_config(manifest.as_ref()) {
                    let _ = update_plugin_window(handle.into(), cx, |plugin, _window, cx| {
                        plugin.attach_dock_agent(config, cx);
                    });
                }
                let _ = handle.update(cx, |_, window, cx| {
                    qingqi_platform::macos::activate_frontmost();
                    cx.activate(true);
                    window.activate_window();
                });
            }
            Err(error) => tracing::warn!(
                plugin_id,
                trace_id = trace.map(|trace| trace.id),
                error = %error,
                "open plugin window failed"
            ),
        }
        log_plugin_window_step(&plugin_id, "open plugin local total", started, trace);
        log_plugin_open_total(
            &plugin_id,
            trace.unwrap_or(PluginOpenTrace { id: 0, started }),
        );
    }

    fn activate_existing_plugin_with_input(
        controller: WindowControllerHandle,
        plugin_id: &str,
        cx: &mut App,
        launch_input: Option<&str>,
    ) -> bool {
        let stored_window_handle = {
            lock_or_recover(&controller, "window_controller")
                .plugin_windows
                .get(plugin_id)
                .copied()
        };
        if let Some(window_handle) = stored_window_handle {
            match update_plugin_window(window_handle, cx, |plugin_window, window, cx| {
                qingqi_platform::macos::activate_frontmost();
                cx.activate(true);
                plugin_window.reopen(window, cx);
                if let Some(input) = launch_input
                    && !input.trim().is_empty()
                {
                    plugin_window.input_changed(input, cx);
                }
                qingqi_platform::macos::restore_window(window);
                window.activate_window();
            }) {
                Ok(_) => {
                    cx.activate(true);
                    return true;
                }
                Err(error) => {
                    tracing::warn!(
                        plugin_id,
                        error = %error,
                        "activate existing plugin window failed"
                    );
                    lock_or_recover(&controller, "window_controller")
                        .clear_plugin_window(plugin_id);
                }
            }
        }

        for window_handle in cx.windows() {
            if plugin_id_from_handle(window_handle, cx).as_deref() != Some(plugin_id) {
                continue;
            }

            let _ = update_plugin_window(window_handle, cx, |plugin_window, window, cx| {
                qingqi_platform::macos::activate_frontmost();
                cx.activate(true);
                plugin_window.reopen(window, cx);
                if let Some(input) = launch_input
                    && !input.trim().is_empty()
                {
                    plugin_window.input_changed(input, cx);
                }
                qingqi_platform::macos::restore_window(window);
                window.activate_window();
            });
            lock_or_recover(&controller, "window_controller")
                .set_plugin_window(plugin_id.to_string(), window_handle);
            cx.activate(true);
            return true;
        }

        false
    }

    fn set_plugin_window(&mut self, plugin_id: impl Into<String>, handle: AnyWindowHandle) {
        let plugin_id = plugin_id.into();
        tracing::debug!(plugin_id, "set plugin window handle");
        self.plugin_windows.insert(plugin_id, handle);
    }

    fn clear_plugin_window(&mut self, plugin_id: &str) {
        tracing::debug!(plugin_id, "clear plugin window handle");
        self.plugin_windows.remove(plugin_id);
    }

    pub fn clear_launcher_window(&mut self) {
        tracing::debug!("clear launcher window handle");
        self.launcher_window = None;
    }

    fn close_idle_plugin(&mut self, plugin_id: &str) {
        lock_or_recover(&self.plugin_manager, "window_controller").close_idle(plugin_id);
        self.clear_plugin_window(plugin_id);
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn handle_dock_agent_event(
        controller: WindowControllerHandle,
        plugin_id: String,
        generation: u64,
        event: DockAgentEvent,
        cx: &mut App,
    ) {
        match event {
            DockAgentEvent::Ready => {
                if lock_or_recover(&controller, "window_controller")
                    .dock_agents
                    .is_current(&plugin_id, generation)
                {
                    tracing::debug!(plugin_id, generation, "Dock agent ready");
                }
            }
            DockAgentEvent::Activate => {
                let current = lock_or_recover(&controller, "window_controller")
                    .dock_agents
                    .is_current(&plugin_id, generation);
                if current {
                    Self::activate_existing_plugin(controller, &plugin_id, cx);
                }
            }
            DockAgentEvent::Quit => {
                let current = lock_or_recover(&controller, "window_controller")
                    .dock_agents
                    .mark_stopping(&plugin_id, generation);
                if current {
                    Self::close_existing_plugin_window(controller, &plugin_id, cx);
                }
            }
            DockAgentEvent::Exited => {
                lock_or_recover(&controller, "window_controller")
                    .dock_agents
                    .handle_exit(&plugin_id, generation, Arc::clone(&controller), cx);
            }
        }
    }
}

fn dock_agent_config(
    manifest: Option<&qingqi_plugin::plugin::Manifest>,
) -> Option<DockAgentConfig> {
    manifest
        .filter(|manifest| manifest.window.show_in_dock)
        .map(DockAgentConfig::from_manifest)
}

fn plugin_reopens_in_active_space(manifest: Option<&qingqi_plugin::plugin::Manifest>) -> bool {
    manifest.is_some_and(|manifest| manifest.window.always_on_top)
}

fn log_plugin_window_step(
    plugin_id: &str,
    step: &'static str,
    started: Instant,
    trace: Option<PluginOpenTrace>,
) {
    let duration_ms = started.elapsed().as_millis() as u64;
    if duration_ms < 50 {
        tracing::debug!(
            plugin_id,
            step,
            duration_ms,
            trace_id = trace.map(|trace| trace.id),
            "plugin window step"
        );
    } else {
        tracing::warn!(
            plugin_id,
            step,
            duration_ms,
            trace_id = trace.map(|trace| trace.id),
            "slow plugin window step"
        );
    }
}

fn log_plugin_open_total(plugin_id: &str, trace: PluginOpenTrace) {
    let duration_ms = trace.started.elapsed().as_millis() as u64;
    if duration_ms < 50 {
        tracing::debug!(
            plugin_id,
            trace_id = trace.id,
            duration_ms,
            "plugin enter total"
        );
    } else {
        tracing::warn!(
            plugin_id,
            trace_id = trace.id,
            duration_ms,
            "slow plugin enter total"
        );
    }
}

fn plugin_window_options(
    title: &str,
    manifest: Option<&qingqi_plugin::plugin::Manifest>,
    display: Option<std::rc::Rc<dyn gpui::PlatformDisplay>>,
    bounds: Bounds<gpui::Pixels>,
) -> WindowOptions {
    // Three flavours of independent plugin window:
    //   • always_on_top = true   → a client-decorated `PopUp` that floats above
    //     other windows (clipboard, anti-peeping). macOS keeps native traffic
    //     lights on the transparent titlebar; other platforms get an overlaid
    //     `ui::window_close_button`.
    //   • background = Blurred    → a Normal window with Blurred background and
    //     client decorations so the titlebar appears transparent and the blur
    //     extends edge-to-edge (SSH client, other glass-style tools).
    //   • otherwise               → an ordinary OS-decorated window with a
    //     native titlebar + close button (full tools that needn't float).
    let always_on_top = manifest.is_some_and(|m| m.window.always_on_top);
    let blurred = manifest.is_some_and(|m| m.window.background == WindowBackgroundSpec::Blurred);

    if always_on_top {
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            display_id: display.map(|display| display.id()),
            titlebar: Some(TitlebarOptions {
                title: Some(title.to_string().into()),
                appears_transparent: true,
                ..Default::default()
            }),
            kind: WindowKind::PopUp,
            is_movable: true,
            is_resizable: false,
            is_minimizable: true,
            window_background: WindowBackgroundAppearance::Blurred,
            window_min_size: Some(bounds.size),
            window_decorations: Some(WindowDecorations::Client),
            ..Default::default()
        }
    } else if blurred {
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            display_id: display.map(|display| display.id()),
            titlebar: Some(TitlebarOptions {
                title: Some(title.to_string().into()),
                appears_transparent: true,
                ..Default::default()
            }),
            kind: WindowKind::Normal,
            is_resizable: true,
            is_minimizable: true,
            window_background: WindowBackgroundAppearance::Blurred,
            window_min_size: Some(bounds.size),
            window_decorations: Some(WindowDecorations::Client),
            ..Default::default()
        }
    } else {
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            display_id: display.map(|display| display.id()),
            titlebar: Some(TitlebarOptions {
                title: Some(title.to_string().into()),
                ..Default::default()
            }),
            kind: WindowKind::Normal,
            is_resizable: true,
            is_minimizable: true,
            window_background: WindowBackgroundAppearance::Opaque,
            window_min_size: Some(bounds.size),
            window_decorations: Some(WindowDecorations::Server),
            ..Default::default()
        }
    }
}

fn plugin_bounds(
    manifest: Option<&qingqi_plugin::plugin::Manifest>,
    cx: &App,
) -> (
    Option<std::rc::Rc<dyn gpui::PlatformDisplay>>,
    Bounds<gpui::Pixels>,
) {
    let Some(manifest) = manifest else {
        return qingqi_platform::display::centered_on_active_display(
            cx,
            size(px(980.0), px(640.0)),
        );
    };
    match manifest.window.size {
        WindowSize::Fixed { width, height } => {
            qingqi_platform::display::centered_on_active_display(cx, size(px(width), px(height)))
        }
        WindowSize::Ratio { width, height } => {
            if let Some(display) = qingqi_platform::display::active_display(cx) {
                let base = display.default_bounds();
                let width = base.size.width * width;
                let height = base.size.height * height;
                let bounds = Bounds::centered_at(display.bounds().center(), size(width, height));
                (Some(display), bounds)
            } else {
                qingqi_platform::display::centered_on_active_display(
                    cx,
                    size(px(1100.0), px(760.0)),
                )
            }
        }
        WindowSize::Auto => {
            // Fall back to a reasonable default size for auto-sized windows.
            qingqi_platform::display::centered_on_active_display(cx, size(px(980.0), px(640.0)))
        }
    }
}

#[cfg(target_os = "windows")]
struct KeepAliveWindow;

#[cfg(target_os = "windows")]
impl Render for KeepAliveWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

fn activate_launcher_window(launcher: &Launcher, window: &mut Window, cx: &mut App) {
    qingqi_platform::macos::activate_frontmost();
    cx.activate(true);
    window.activate_window();
    launcher.focus_query_input(window, cx);
    // Ensure blink cursor starts even if on_focus callback has timing issues
    if let Some(input) = launcher.query_input.as_ref() {
        input.update(cx, |_input, _cx| {
            // input.start_blink_cursor(cx); // method not available in this gpui version
        });
    }
    window.defer(cx, |window, cx| {
        qingqi_platform::macos::activate_frontmost();
        cx.activate(true);
        window.activate_window();
    });
}

fn update_launcher<R>(
    window_handle: AnyWindowHandle,
    cx: &mut App,
    f: impl FnOnce(&mut Launcher, &mut Window, &mut Context<Launcher>) -> R,
) -> Result<R, anyhow::Error> {
    if let Some(handle) = window_handle.downcast::<Launcher>() {
        return handle.update(cx, f);
    }
    let root_handle = window_handle
        .downcast::<Root>()
        .ok_or_else(|| anyhow::anyhow!("unexpected window root"))?;
    root_handle
        .update(cx, |root, window, cx| -> Result<R, anyhow::Error> {
            let launcher = root
                .view()
                .clone()
                .downcast::<Launcher>()
                .map_err(|_| anyhow::anyhow!("Root does not wrap Launcher"))?;
            Ok(launcher.update(cx, |launcher, cx| f(launcher, window, cx)))
        })
        .map_err(|error| anyhow::anyhow!("{error}"))?
}

fn plugin_id_from_handle(window_handle: AnyWindowHandle, cx: &mut App) -> Option<String> {
    if let Some(handle) = window_handle.downcast::<PluginWindow>() {
        return handle
            .update(cx, |plugin, _, _| plugin.plugin_id.clone())
            .ok();
    }
    window_handle
        .downcast::<Root>()
        .and_then(|root| {
            root.update(cx, |root, _, cx| {
                root.view()
                    .clone()
                    .downcast::<PluginWindow>()
                    .ok()
                    .map(|entity| entity.read(cx).plugin_id.clone())
            })
            .ok()
        })
        .flatten()
}

fn update_plugin_window<R>(
    window_handle: AnyWindowHandle,
    cx: &mut App,
    f: impl FnOnce(&mut PluginWindow, &mut Window, &mut Context<PluginWindow>) -> R,
) -> Result<R, anyhow::Error> {
    if let Some(handle) = window_handle.downcast::<PluginWindow>() {
        return handle.update(cx, f);
    }
    let root_handle = window_handle
        .downcast::<Root>()
        .ok_or_else(|| anyhow::anyhow!("unexpected window root"))?;
    root_handle
        .update(cx, |root, window, cx| -> Result<R, anyhow::Error> {
            let plugin = root
                .view()
                .clone()
                .downcast::<PluginWindow>()
                .map_err(|_| anyhow::anyhow!("Root does not wrap PluginWindow"))?;
            Ok(plugin.update(cx, |plugin, cx| f(plugin, window, cx)))
        })
        .map_err(|error| anyhow::anyhow!("{error}"))?
}

struct PluginWindow {
    controller: WindowControllerHandle,
    view: Option<Box<dyn WindowView>>,
    plugin_id: String,
    /// Whether this window uses client decorations. Non-macOS platforms need
    /// an overlaid close button; macOS keeps native traffic lights.
    client_drawn: bool,
    /// Set to true after the app-aware on_release hook has run, so that
    /// `Drop` does not redundantly invoke `on_close` / `close_idle_plugin`.
    closed: bool,
    dock_agent_registered: bool,
}

impl PluginWindow {
    fn new(
        cx: &Context<Self>,
        controller: WindowControllerHandle,
        view: Box<dyn WindowView>,
        client_drawn: bool,
    ) -> Self {
        let plugin_id = view.plugin_id().to_string();
        let controller2 = Arc::clone(&controller);
        let plugin_id2 = plugin_id.clone();
        cx.on_release(move |this: &mut Self, cx: &mut App| {
            if this.closed {
                return;
            }
            this.closed = true;
            if let Some(mut view) = this.view.take() {
                view.on_close_with_app(cx);
            }
            let mut controller = lock_or_recover(&controller2, "window_controller");
            controller.close_idle_plugin(&plugin_id2);
            if this.dock_agent_registered {
                controller.dock_agents.release(&plugin_id2);
            }
        })
        .detach();
        Self {
            controller,
            view: Some(view),
            plugin_id,
            client_drawn,
            closed: false,
            dock_agent_registered: false,
        }
    }

    fn attach_dock_agent(&mut self, config: DockAgentConfig, cx: &mut Context<Self>) {
        if self.dock_agent_registered {
            return;
        }
        let controller = Arc::clone(&self.controller);
        lock_or_recover(&self.controller, "window_controller")
            .dock_agents
            .acquire(config, controller, cx);
        self.dock_agent_registered = true;
    }

    fn reopen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = self.view.as_mut() {
            view.on_reopen(window, cx);
        }
    }

    fn input_changed(&mut self, text: &str, cx: &mut Context<Self>) {
        if let Some(view) = self.view.as_mut() {
            view.on_input_changed(text, cx);
        }
    }
}

impl Render for PluginWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let client_drawn = self.client_drawn;
        let content = self
            .view
            .as_mut()
            .map(|view| view.render(window, cx))
            .unwrap_or_else(|| div().child("插件已关闭").into_any_element());

        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .size_full()
            .relative()
            .capture_key_down(
                cx.listener(|_this, event: &gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.key.as_str() == "escape" {
                        window.defer(cx, |window, _cx| window.remove_window());
                        cx.stop_propagation();
                    }
                }),
            )
            .child(content)
            // Client-drawn (always-on-top) windows have no OS titlebar on
            // non-macOS platforms, so overlay a close button. macOS keeps the
            // native traffic lights even with a transparent titlebar.
            .children((client_drawn && !cfg!(target_os = "macos")).then(|| {
                div()
                    .absolute()
                    .top(px(6.0))
                    .right(px(6.0))
                    .child(ui::window_close_button(cx))
            }))
            .children(notification_layer)
    }
}

impl Drop for PluginWindow {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        if let Some(mut view) = self.view.take() {
            view.on_close();
        }
        let mut controller = lock_or_recover(&self.controller, "window_controller");
        controller.close_idle_plugin(&self.plugin_id);
        if self.dock_agent_registered {
            controller.dock_agents.release(&self.plugin_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{App, Context, TestAppContext, Window};
    use qingqi_plugin::events::AppEventBus;
    use qingqi_plugin::plugin::PluginId;

    use crate::app::app_catalog::AppCatalog;
    use crate::app::app_index::AppIndexService;
    use qingqi_core::plugin::PluginManager;
    use qingqi_plugin::plugin::WindowView;

    use super::*;

    /// Helper to build a WindowController suitable for [`PluginWindow`] tests.
    fn test_controller() -> WindowControllerHandle {
        let paths = qingqi_plugin::storage::AppPaths::for_test("/tmp/qingqi-test-ignore");
        let database = std::sync::Arc::new(qingqi_plugin::database::DatabaseService::new(paths));
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::feature(
                "app-launcher",
                "index",
                "app-launcher/index.db",
            ))
            .unwrap();
        let usage_store =
            qingqi_core::command_usage::CommandUsageStore::new(database.clone(), "command-usage");
        let app_index =
            std::sync::Arc::new(AppIndexService::new(database.clone(), usage_store.clone()));
        let app_catalog = std::sync::Arc::new(AppCatalog::new(app_index));
        let events = AppEventBus::new();
        let command_catalog_store = qingqi_core::command_catalog::CommandCatalogStore::new(
            database,
            qingqi_core::command_catalog::COMMAND_CATALOG_KEY,
        );
        let plugin_manager = PluginManager::new(events, usage_store, command_catalog_store);
        std::sync::Arc::new(Mutex::new(WindowController::new(
            std::sync::Arc::new(Mutex::new(plugin_manager)),
            app_catalog,
            AppEventBus::new(),
        )))
    }

    /// A [`WindowView`] that counts how many times its app-aware close hook fires.
    struct CountingAppCloseView {
        plugin_id: PluginId,
        count: Rc<RefCell<usize>>,
    }

    impl WindowView for CountingAppCloseView {
        fn plugin_id(&self) -> PluginId {
            self.plugin_id.clone()
        }

        fn title(&self) -> std::sync::Arc<str> {
            "Test".into()
        }

        fn render(&mut self, _window: &mut Window, _cx: &mut App) -> gpui::AnyElement {
            gpui::div().into_any_element()
        }

        fn on_close_with_app(&mut self, _cx: &mut App) {
            *self.count.borrow_mut() += 1;
        }
    }

    /// A [`WindowView`] that only implements the legacy `on_close()` to prove
    /// the default `on_close_with_app` forwards correctly.
    struct LegacyCloseView {
        plugin_id: PluginId,
        count: Rc<RefCell<usize>>,
    }

    impl WindowView for LegacyCloseView {
        fn plugin_id(&self) -> PluginId {
            self.plugin_id.clone()
        }

        fn title(&self) -> std::sync::Arc<str> {
            "Legacy".into()
        }

        fn render(&mut self, _window: &mut Window, _cx: &mut App) -> gpui::AnyElement {
            gpui::div().into_any_element()
        }

        fn on_close(&mut self) {
            *self.count.borrow_mut() += 1;
        }
    }

    /// A trivial slot view that holds an optional child entity. Dropping the
    /// child from the slot triggers GPUI's standard release machinery — the
    /// same path `PluginWindow` takes when its containing window closes.
    struct SlotView {
        child: Option<gpui::Entity<PluginWindow>>,
    }

    impl gpui::Render for SlotView {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            gpui::div()
        }
    }

    #[gpui::test]
    fn plugin_window_entity_release_calls_app_aware_hook_once(cx: &mut TestAppContext) {
        let count = Rc::new(RefCell::new(0));
        let count_clone = Rc::clone(&count);
        let controller = test_controller();

        let (slot_entity, cx) = cx.add_window_view(|_window, cx| {
            let plugin_entity = cx.new(|cx| {
                PluginWindow::new(
                    cx,
                    controller,
                    Box::new(CountingAppCloseView {
                        plugin_id: "test.app-close".into(),
                        count: count_clone,
                    }),
                    false,
                )
            });
            SlotView {
                child: Some(plugin_entity),
            }
        });

        // Clear the slot so GPUI releases the PluginWindow on the next flush.
        slot_entity.update(cx, |slot, _cx| {
            slot.child = None;
        });
        cx.update(|_window, _cx| {});

        assert_eq!(
            *count.borrow(),
            1,
            "app-aware close hook must fire exactly once on entity release"
        );
    }

    #[gpui::test]
    fn plugin_window_legacy_on_close_forwards_to_app_aware_hook(cx: &mut TestAppContext) {
        let count = Rc::new(RefCell::new(0));
        let count_clone = Rc::clone(&count);
        let controller = test_controller();

        let (slot_entity, cx) = cx.add_window_view(|_window, cx| {
            let plugin_entity = cx.new(|cx| {
                PluginWindow::new(
                    cx,
                    controller,
                    Box::new(LegacyCloseView {
                        plugin_id: "test.legacy".into(),
                        count: count_clone,
                    }),
                    false,
                )
            });
            SlotView {
                child: Some(plugin_entity),
            }
        });

        slot_entity.update(cx, |slot, _cx| {
            slot.child = None;
        });
        cx.update(|_window, _cx| {});

        assert_eq!(
            *count.borrow(),
            1,
            "legacy on_close must still be invoked via default on_close_with_app"
        );
    }
}

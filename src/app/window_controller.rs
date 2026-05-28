use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Instant,
};

use gpui::{
    AnyWindowHandle, App, Bounds, Context, Focusable, IntoElement, ParentElement, Render, Styled,
    TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowDecorations,
    WindowKind, WindowOptions, div, point, prelude::*, px, size,
};

use crate::{
    app::{launcher::Launcher, text_input::TextInput},
    core::{
        command::{CommandInvocation, CommandTarget},
        plugin::{PluginManager, PluginSession},
        plugin_spec::WindowSize,
    },
    features::clipboard::service::ClipboardService,
    platform,
};

pub type WindowControllerHandle = Rc<RefCell<WindowController>>;

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
    plugin_manager: Rc<RefCell<PluginManager>>,
    clipboard_service: Arc<Mutex<ClipboardService>>,
    launcher_window: Option<AnyWindowHandle>,
    plugin_windows: HashMap<String, AnyWindowHandle>,
}

impl WindowController {
    pub fn new(
        plugin_manager: Rc<RefCell<PluginManager>>,
        clipboard_service: Arc<Mutex<ClipboardService>>,
    ) -> Self {
        Self {
            plugin_manager,
            clipboard_service,
            launcher_window: None,
            plugin_windows: HashMap::new(),
        }
    }

    pub fn plugin_manager(&self) -> Rc<RefCell<PluginManager>> {
        Rc::clone(&self.plugin_manager)
    }

    pub fn toggle_launcher(controller: WindowControllerHandle, cx: &mut App) {
        let stored_window_handle = { controller.borrow().launcher_window };
        if let Some(window_handle) = stored_window_handle {
            if let Some(handle) = window_handle.downcast::<Launcher>() {
                match handle.update(cx, |_, window, _| window.remove_window()) {
                    Ok(_) => {
                        controller.borrow_mut().launcher_window = None;
                        return;
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "toggle existing launcher window failed");
                        controller.borrow_mut().launcher_window = None;
                    }
                }
            } else {
                tracing::warn!("stored launcher window handle had unexpected root type");
                controller.borrow_mut().launcher_window = None;
            }
        }

        Self::show_launcher(controller, cx);
    }

    pub fn show_launcher(controller: WindowControllerHandle, cx: &mut App) {
        let stored_window_handle = { controller.borrow().launcher_window };
        if let Some(window_handle) = stored_window_handle {
            if let Some(handle) = window_handle.downcast::<Launcher>() {
                cx.activate(true);
                match handle.update(cx, |_, window, _| window.activate_window()) {
                    Ok(_) => {
                        cx.activate(true);
                        return;
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "activate existing launcher window failed");
                        controller.borrow_mut().launcher_window = None;
                    }
                }
            } else {
                tracing::warn!("stored launcher window handle had unexpected root type");
                controller.borrow_mut().launcher_window = None;
            }
        }

        Self::open_launcher(controller, cx);
    }

    fn open_launcher(controller: WindowControllerHandle, cx: &mut App) {
        let plugin_manager = controller.borrow().plugin_manager();
        let clipboard_service = Arc::clone(&controller.borrow().clipboard_service);
        let initial_results = plugin_manager
            .borrow_mut()
            .commands_with_clipboard(Launcher::latest_clipboard_context_kinds(&clipboard_service))
            .len();
        let window_size = size(
            px(Launcher::window_width()),
            px(Launcher::window_height_for_results(initial_results)),
        );
        let window_min_size = size(
            px(Launcher::window_width()),
            px(Launcher::min_window_height()),
        );
        let (display, bounds) = platform::display::centered_on_active_display(cx, window_size);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            display_id: display.map(|display| display.id()),
            titlebar: Some(TitlebarOptions {
                title: Some("Qingqi".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(-80.0), px(-80.0))),
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
        let controller_for_entity = Rc::clone(&controller);
        match cx.open_window(options, move |window, cx| {
            window.set_window_title("Qingqi");
            let query_input = cx.new(|cx| {
                let mut input = TextInput::new(cx, "搜索工具、命令、文件...", "");
                Launcher::configure_query_input(&mut input, cx);
                input
            });
            let clipboard_service = Arc::clone(&clipboard_service);
            let launcher =
                cx.new(|cx| Launcher::new(Rc::clone(&plugin_manager), clipboard_service, cx));
            let handle = launcher.clone();
            launcher.update(cx, |launcher, launcher_cx| {
                launcher.attach_handle(handle);
                launcher.attach_window_controller(Rc::clone(&controller_for_entity));
                launcher.attach_query_input(query_input.clone());
                launcher.observe_query_input(launcher_cx);
                launcher.initialize_async(launcher_cx);
            });
            window.focus(&query_input.focus_handle(cx));
            launcher
        }) {
            Ok(handle) => {
                controller.borrow_mut().launcher_window = Some(handle.into());
                cx.activate(true);
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
        let plugin_manager = controller.borrow().plugin_manager();
        let manifest = plugin_manager
            .borrow()
            .manifests()
            .into_iter()
            .find(|manifest| manifest.id == plugin_id);

        if plugin_reopens_in_active_space(manifest) {
            let close_started = Instant::now();
            Self::close_existing_plugin_window(Rc::clone(&controller), &plugin_id, cx);
            log_plugin_window_step(
                &plugin_id,
                "close existing plugin window",
                close_started,
                trace,
            );
        } else if Self::activate_existing_plugin(Rc::clone(&controller), &plugin_id, cx) {
            log_plugin_window_step(&plugin_id, "activate existing plugin", started, trace);
            log_plugin_open_total(
                &plugin_id,
                trace.unwrap_or(PluginOpenTrace { id: 0, started }),
            );
            return;
        }

        let session_started = Instant::now();
        let session = match plugin_manager.borrow_mut().open_session(&plugin_id, cx) {
            Ok(session) => session,
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
        log_plugin_window_step(&plugin_id, "open plugin session", session_started, trace);

        let title = session.title();
        let (display, bounds) = plugin_bounds(manifest, cx);
        let options = plugin_window_options(title, manifest, display, bounds);
        let plugin_id_for_window = plugin_id.clone();
        let controller_for_window = Rc::clone(&controller);
        let window_started = Instant::now();
        match cx.open_window(options, move |window, cx| {
            window.set_window_title(title);
            cx.new(|_| PluginWindow::new(Rc::clone(&controller_for_window), session))
        }) {
            Ok(handle) => {
                log_plugin_window_step(&plugin_id, "open plugin window", window_started, trace);
                controller
                    .borrow_mut()
                    .set_plugin_window(plugin_id_for_window, handle.into());
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
        let stored_window_handle = { controller.borrow().plugin_windows.get(plugin_id).copied() };
        if let Some(window_handle) = stored_window_handle {
            if let Some(handle) = window_handle.downcast::<PluginWindow>() {
                match handle.update(cx, |plugin_window, window, cx| {
                    cx.activate(true);
                    plugin_window.reopen(window, cx);
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
                        controller.borrow_mut().clear_plugin_window(plugin_id);
                    }
                }
            } else {
                tracing::warn!(
                    plugin_id,
                    "stored plugin window handle had unexpected root type"
                );
                controller.borrow_mut().clear_plugin_window(plugin_id);
            }
        }

        for window_handle in cx.windows() {
            let Some(handle) = window_handle.downcast::<PluginWindow>() else {
                continue;
            };
            let is_same_plugin = handle
                .read(cx)
                .map(|plugin_window| plugin_window.plugin_id == plugin_id)
                .unwrap_or(false);
            if !is_same_plugin {
                continue;
            }

            let _ = handle.update(cx, |plugin_window, window, cx| {
                cx.activate(true);
                plugin_window.reopen(window, cx);
                window.activate_window();
            });
            controller
                .borrow_mut()
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
        let stored_window_handle = { controller.borrow().plugin_windows.get(plugin_id).copied() };
        if let Some(window_handle) = stored_window_handle {
            if let Some(handle) = window_handle.downcast::<PluginWindow>() {
                match handle.update(cx, |_, window, _| window.remove_window()) {
                    Ok(_) => {
                        controller.borrow_mut().clear_plugin_window(plugin_id);
                        return true;
                    }
                    Err(error) => {
                        tracing::warn!(
                            plugin_id,
                            error = %error,
                            "close existing plugin window failed"
                        );
                        controller.borrow_mut().clear_plugin_window(plugin_id);
                    }
                }
            } else {
                tracing::warn!(
                    plugin_id,
                    "stored plugin window handle had unexpected root type"
                );
                controller.borrow_mut().clear_plugin_window(plugin_id);
            }
        }

        for window_handle in cx.windows() {
            let Some(handle) = window_handle.downcast::<PluginWindow>() else {
                continue;
            };
            let is_same_plugin = handle
                .read(cx)
                .map(|plugin_window| plugin_window.plugin_id == plugin_id)
                .unwrap_or(false);
            if !is_same_plugin {
                continue;
            }

            let closed = handle
                .update(cx, |_, window, _| window.remove_window())
                .is_ok();
            controller.borrow_mut().clear_plugin_window(plugin_id);
            if closed {
                return true;
            }
        }

        false
    }

    pub fn run_command(
        controller: WindowControllerHandle,
        target: CommandTarget,
        cx: &mut App,
    ) -> Option<String> {
        Self::run_command_with_trace(controller, target, cx, None)
    }

    pub fn run_command_with_trace(
        controller: WindowControllerHandle,
        target: CommandTarget,
        cx: &mut App,
        trace: Option<PluginOpenTrace>,
    ) -> Option<String> {
        match target {
            CommandTarget::PluginOpen { plugin_id } => {
                Self::open_plugin_with_trace(controller, plugin_id, cx, trace);
                None
            }
            target @ CommandTarget::PluginAction { .. } => {
                let plugin_manager = controller.borrow().plugin_manager();
                plugin_manager
                    .borrow_mut()
                    .handle_command(CommandInvocation { target }, cx)
                    .ok()
                    .and_then(|outcome| outcome.message)
            }
        }
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
        self.plugin_manager.borrow_mut().close_idle(plugin_id);
        self.clear_plugin_window(plugin_id);
    }
}

fn plugin_reopens_in_active_space(manifest: Option<crate::core::plugin::PluginManifest>) -> bool {
    manifest.is_some_and(|manifest| manifest.visual.window.always_on_top)
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
        tracing::info!(
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
    title: &'static str,
    manifest: Option<crate::core::plugin::PluginManifest>,
    display: Option<std::rc::Rc<dyn gpui::PlatformDisplay>>,
    bounds: Bounds<gpui::Pixels>,
) -> WindowOptions {
    let Some(manifest) = manifest else {
        return WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            display_id: display.map(|display| display.id()),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                ..Default::default()
            }),
            ..Default::default()
        };
    };

    let always_on_top = manifest.visual.window.always_on_top;
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        display_id: display.map(|display| display.id()),
        titlebar: Some(TitlebarOptions {
            title: Some(title.into()),
            appears_transparent: false,
            ..Default::default()
        }),
        kind: if always_on_top {
            WindowKind::PopUp
        } else {
            WindowKind::Normal
        },
        is_resizable: !always_on_top,
        is_minimizable: true,
        window_background: WindowBackgroundAppearance::Opaque,
        window_min_size: always_on_top.then_some(bounds.size),
        ..Default::default()
    }
}

fn plugin_bounds(
    manifest: Option<crate::core::plugin::PluginManifest>,
    cx: &App,
) -> (
    Option<std::rc::Rc<dyn gpui::PlatformDisplay>>,
    Bounds<gpui::Pixels>,
) {
    let Some(manifest) = manifest else {
        return platform::display::centered_on_active_display(cx, size(px(980.0), px(640.0)));
    };
    match manifest.visual.window.size {
        WindowSize::Fixed { width, height } => {
            platform::display::centered_on_active_display(cx, size(px(width), px(height)))
        }
        WindowSize::Ratio { width, height } => {
            if let Some(display) = platform::display::active_display(cx) {
                let base = display.default_bounds();
                let width = base.size.width * width;
                let height = base.size.height * height;
                let bounds = Bounds::centered_at(display.bounds().center(), size(width, height));
                (Some(display), bounds)
            } else {
                platform::display::centered_on_active_display(cx, size(px(1100.0), px(760.0)))
            }
        }
    }
}

struct PluginWindow {
    controller: WindowControllerHandle,
    session: Option<Box<dyn PluginSession>>,
    plugin_id: String,
}

impl PluginWindow {
    fn new(controller: WindowControllerHandle, session: Box<dyn PluginSession>) -> Self {
        let plugin_id = session.plugin_id().to_string();
        Self {
            controller,
            session: Some(session),
            plugin_id,
        }
    }

    fn reopen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(session) = self.session.as_mut() {
            session.on_reopen(window, cx);
        }
    }
}

impl Render for PluginWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self
            .session
            .as_mut()
            .map(|session| session.render(window, cx))
            .unwrap_or_else(|| div().child("插件已关闭").into_any_element());

        div().size_full().child(content)
    }
}

impl Drop for PluginWindow {
    fn drop(&mut self) {
        if let Some(mut session) = self.session.take() {
            session.on_close();
        }
        self.controller
            .borrow_mut()
            .close_idle_plugin(&self.plugin_id);
    }
}

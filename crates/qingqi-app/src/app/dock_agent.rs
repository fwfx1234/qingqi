use std::{
    ffi::OsString,
    io::{BufRead, BufReader, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context as _, Result, anyhow};
use gpui::{
    AnyWindowHandle, AppContext, Application, Bounds, Context, Global, IntoElement, Render, Window,
    WindowBackgroundAppearance, WindowBounds, WindowDecorations, WindowKind, WindowOptions, div,
    point, px, size,
};
use qingqi_plugin::plugin_spec::PluginAccent;
use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
use std::{
    collections::HashMap,
    process::{Child, ChildStdin, Command, Stdio},
};

#[cfg(target_os = "macos")]
use gpui::App;

#[cfg(target_os = "macos")]
use crate::app::window_controller::{WindowController, WindowControllerHandle};

pub const DOCK_AGENT_ARG: &str = "--qingqi-dock-agent";
pub const EVENT_READY: &str = "ready";
pub const EVENT_ACTIVATE: &str = "activate";
pub const EVENT_QUIT: &str = "quit";
pub const COMMAND_SHUTDOWN: &str = "shutdown";

struct DockAgentKeepAlive;

struct DockAgentKeepAliveGlobal {
    _handle: AnyWindowHandle,
}

impl Global for DockAgentKeepAliveGlobal {}

impl Render for DockAgentKeepAlive {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockAgentConfig {
    pub plugin_id: String,
    pub plugin_name: String,
    pub icon: String,
    pub accent: PluginAccent,
}

impl DockAgentConfig {
    pub fn from_manifest(manifest: &qingqi_plugin::plugin::Manifest) -> Self {
        Self {
            plugin_id: manifest.id.to_string(),
            plugin_name: manifest.name.to_string(),
            icon: manifest.icon.as_str().to_string(),
            accent: manifest
                .visual
                .as_ref()
                .map(|visual| visual.accent)
                .unwrap_or(PluginAccent::Blue),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum DockAgentEvent {
    Ready,
    Activate,
    Quit,
    Exited,
}

#[cfg(target_os = "macos")]
struct DockAgentProcess {
    child: Child,
    input: ChildStdin,
}

#[cfg(target_os = "macos")]
struct DockAgentEntry {
    config: DockAgentConfig,
    leases: usize,
    generation: u64,
    restart_attempts: u8,
    stopping: bool,
    process: Option<DockAgentProcess>,
}

#[cfg(target_os = "macos")]
#[derive(Default)]
pub(crate) struct DockAgentManager {
    agents: HashMap<String, DockAgentEntry>,
    next_generation: u64,
}

#[cfg(not(target_os = "macos"))]
#[derive(Default)]
pub(crate) struct DockAgentManager;

#[cfg(target_os = "macos")]
impl DockAgentManager {
    pub fn acquire(
        &mut self,
        config: DockAgentConfig,
        controller: WindowControllerHandle,
        cx: &mut App,
    ) {
        if let Some(entry) = self.agents.get_mut(&config.plugin_id) {
            entry.leases = entry.leases.saturating_add(1);
            if entry.process.is_none() && !entry.stopping {
                self.ensure_process(&config.plugin_id, controller, cx);
            }
            return;
        }

        let plugin_id = config.plugin_id.clone();
        self.agents.insert(
            plugin_id.clone(),
            DockAgentEntry {
                config,
                leases: 1,
                generation: 0,
                restart_attempts: 0,
                stopping: false,
                process: None,
            },
        );
        self.ensure_process(&plugin_id, controller, cx);
    }

    pub fn release(&mut self, plugin_id: &str) {
        let Some(entry) = self.agents.get_mut(plugin_id) else {
            return;
        };
        entry.leases = entry.leases.saturating_sub(1);
        if entry.leases > 0 {
            return;
        }
        if let Some(mut entry) = self.agents.remove(plugin_id)
            && let Some(process) = entry.process.take()
        {
            stop_process(process);
        }
    }

    pub fn is_current(&self, plugin_id: &str, generation: u64) -> bool {
        self.agents
            .get(plugin_id)
            .is_some_and(|entry| entry.generation == generation && !entry.stopping)
    }

    pub fn mark_stopping(&mut self, plugin_id: &str, generation: u64) -> bool {
        let Some(entry) = self.agents.get_mut(plugin_id) else {
            return false;
        };
        if entry.generation != generation {
            return false;
        }
        entry.stopping = true;
        true
    }

    pub fn handle_exit(
        &mut self,
        plugin_id: &str,
        generation: u64,
        controller: WindowControllerHandle,
        cx: &mut App,
    ) {
        let Some(entry) = self.agents.get_mut(plugin_id) else {
            return;
        };
        if entry.generation != generation {
            return;
        }
        if let Some(process) = entry.process.take() {
            reap_process(process);
        }
        if entry.stopping || entry.leases == 0 {
            return;
        }
        if entry.restart_attempts >= 1 {
            tracing::warn!(
                plugin_id,
                "Dock agent exited after restart; leaving window open"
            );
            return;
        }
        entry.restart_attempts += 1;
        tracing::warn!(plugin_id, "Dock agent exited unexpectedly; restarting once");
        self.ensure_process(plugin_id, controller, cx);
    }

    fn ensure_process(
        &mut self,
        plugin_id: &str,
        controller: WindowControllerHandle,
        cx: &mut App,
    ) {
        let Some(entry) = self.agents.get(plugin_id) else {
            return;
        };
        if entry.process.is_some() || entry.stopping {
            return;
        }
        let config = entry.config.clone();
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let generation = self.next_generation;
        match spawn_process(&config, generation, controller, cx) {
            Ok(process) => {
                if let Some(entry) = self.agents.get_mut(plugin_id) {
                    entry.generation = generation;
                    entry.process = Some(process);
                }
            }
            Err(error) => {
                tracing::warn!(plugin_id, error = %error, "failed to start Dock agent");
            }
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for DockAgentManager {
    fn drop(&mut self) {
        for (_, mut entry) in self.agents.drain() {
            if let Some(process) = entry.process.take() {
                stop_process(process);
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl DockAgentManager {
    pub fn acquire(
        &mut self,
        _config: DockAgentConfig,
        _controller: crate::app::window_controller::WindowControllerHandle,
        _cx: &mut gpui::App,
    ) {
    }

    pub fn release(&mut self, _plugin_id: &str) {}
}

#[cfg(target_os = "macos")]
fn spawn_process(
    config: &DockAgentConfig,
    generation: u64,
    controller: WindowControllerHandle,
    cx: &mut App,
) -> Result<DockAgentProcess> {
    let executable = std::env::current_exe().context("cannot locate Qingqi executable")?;
    let argument = config_argument(config)?;
    let mut child = Command::new(executable)
        .arg(DOCK_AGENT_ARG)
        .arg(argument)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("cannot spawn Dock agent")?;
    let input = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("Dock agent stdin is unavailable"))?;
    let output = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Dock agent stdout is unavailable"))?;

    let plugin_id = config.plugin_id.clone();
    let reader = Arc::new(Mutex::new(BufReader::new(output)));
    cx.spawn(async move |async_cx| {
        loop {
            let reader = Arc::clone(&reader);
            let line = async_cx
                .background_executor()
                .spawn(async move {
                    let mut line = String::new();
                    let bytes = reader.lock().ok()?.read_line(&mut line).ok()?;
                    Some((bytes, line))
                })
                .await;
            let event = match line {
                Some((0, _)) | None => DockAgentEvent::Exited,
                Some((_, line)) => match parse_agent_event(&line) {
                    Some(event) => event,
                    None => {
                        tracing::debug!(plugin_id, line, "ignoring unknown Dock agent event");
                        continue;
                    }
                },
            };
            let controller = Arc::clone(&controller);
            let plugin_id_for_event = plugin_id.clone();
            let _ = async_cx.update(move |cx| {
                WindowController::handle_dock_agent_event(
                    controller,
                    plugin_id_for_event,
                    generation,
                    event,
                    cx,
                );
            });
            if event == DockAgentEvent::Exited {
                break;
            }
        }
    })
    .detach();

    Ok(DockAgentProcess { child, input })
}

#[cfg(target_os = "macos")]
fn stop_process(mut process: DockAgentProcess) {
    let _ = writeln!(process.input, "{COMMAND_SHUTDOWN}");
    let _ = process.input.flush();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if process.child.try_wait().ok().flatten().is_none() {
            let _ = process.child.kill();
        }
        let _ = process.child.wait();
    });
}

#[cfg(target_os = "macos")]
fn reap_process(mut process: DockAgentProcess) {
    std::thread::spawn(move || {
        let _ = process.child.wait();
    });
}

pub fn config_from_args(
    args: impl IntoIterator<Item = OsString>,
) -> Result<Option<DockAgentConfig>> {
    let mut args = args.into_iter();
    let _executable = args.next();
    let Some(mode) = args.next() else {
        return Ok(None);
    };
    if mode != DOCK_AGENT_ARG {
        return Ok(None);
    }
    let config = args
        .next()
        .ok_or_else(|| anyhow!("missing Dock agent configuration"))?;
    let config = config
        .into_string()
        .map_err(|_| anyhow!("Dock agent configuration is not valid UTF-8"))?;
    serde_json::from_str(&config)
        .context("invalid Dock agent configuration")
        .map(Some)
}

pub fn config_argument(config: &DockAgentConfig) -> Result<String> {
    serde_json::to_string(config).context("cannot encode Dock agent configuration")
}

pub fn run(config: DockAgentConfig) -> Result<()> {
    qingqi_platform::macos::prepare_dock_agent_name(&config.plugin_name);
    let app = Application::new();
    let icon_bytes = qingqi_ui::assets::embedded(&config.icon)
        .ok_or_else(|| anyhow!("Dock agent icon asset not found: {}", config.icon))?;
    if !config.icon.ends_with(".svg") {
        return Err(anyhow!(
            "Dock agent currently requires an SVG icon: {}",
            config.icon
        ));
    }
    let (foreground, background) = accent_colors(config.accent);
    let icon_png =
        qingqi_platform::icon_raster::rasterize_dock_icon(icon_bytes, 512, foreground, background)
            .map_err(|error| anyhow!(error))?;
    let output = Arc::new(Mutex::new(std::io::stdout()));
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let startup_error = Arc::new(Mutex::new(None));
    app.on_reopen({
        let output = Arc::clone(&output);
        move |_cx| send_event(&output, EVENT_ACTIVATE)
    });

    let startup_error_for_run = Arc::clone(&startup_error);
    app.run(move |cx| {
        let keep_alive_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(-10_000.0), px(-10_000.0)),
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
        let keep_alive = match cx.open_window(keep_alive_options, |_window, cx| {
            cx.new(|_| DockAgentKeepAlive)
        }) {
            Ok(handle) => handle,
            Err(error) => {
                if let Ok(mut startup_error) = startup_error_for_run.lock() {
                    *startup_error = Some(format!(
                        "cannot create Dock agent keep-alive window: {error}"
                    ));
                }
                cx.quit();
                return;
            }
        };
        cx.set_global(DockAgentKeepAliveGlobal {
            _handle: keep_alive.into(),
        });
        let output_on_quit = Arc::clone(&output);
        let shutdown_on_quit = Arc::clone(&shutdown_requested);
        cx.on_app_quit(move |_cx| {
            if !shutdown_on_quit.load(Ordering::SeqCst) {
                send_event(&output_on_quit, EVENT_QUIT);
            }
            async {}
        })
        .detach();

        let input = Arc::new(Mutex::new(BufReader::new(std::io::stdin())));
        let shutdown_for_input = Arc::clone(&shutdown_requested);
        cx.spawn(async move |async_cx| {
            loop {
                let input = Arc::clone(&input);
                let command = async_cx
                    .background_executor()
                    .spawn(async move {
                        let mut line = String::new();
                        let bytes = input.lock().ok()?.read_line(&mut line).ok()?;
                        Some((bytes, line))
                    })
                    .await;
                let should_quit = match command {
                    Some((0, _)) | None => parent_requests_shutdown(None),
                    Some((_, line)) => parent_requests_shutdown(Some(&line)),
                };
                if should_quit {
                    shutdown_for_input.store(true, Ordering::SeqCst);
                    let _ = async_cx.update(|cx| cx.quit());
                    break;
                }
            }
        })
        .detach();

        let output_on_ready = Arc::clone(&output);
        cx.defer(move |cx| {
            match qingqi_platform::macos::configure_dock_agent(&config.plugin_name, &icon_png) {
                Ok(()) => send_event(&output_on_ready, EVENT_READY),
                Err(error) => {
                    if let Ok(mut startup_error) = startup_error_for_run.lock() {
                        *startup_error = Some(error);
                    }
                    cx.quit();
                }
            }
        });
    });
    if let Some(error) = startup_error.lock().ok().and_then(|mut error| error.take()) {
        return Err(anyhow!(error));
    }
    Ok(())
}

fn send_event(output: &Arc<Mutex<std::io::Stdout>>, event: &str) {
    let Ok(mut output) = output.lock() else {
        return;
    };
    let _ = writeln!(output, "{event}");
    let _ = output.flush();
}

#[allow(dead_code)]
fn parse_agent_event(line: &str) -> Option<DockAgentEvent> {
    match line.trim() {
        EVENT_READY => Some(DockAgentEvent::Ready),
        EVENT_ACTIVATE => Some(DockAgentEvent::Activate),
        EVENT_QUIT => Some(DockAgentEvent::Quit),
        _ => None,
    }
}

fn parent_requests_shutdown(line: Option<&str>) -> bool {
    line.is_none_or(|line| line.trim() == COMMAND_SHUTDOWN)
}

fn accent_colors(accent: PluginAccent) -> ([u8; 3], [u8; 3]) {
    use PluginAccent::*;
    match accent {
        Blue => ([59, 130, 246], [219, 234, 254]),
        Cyan => ([14, 165, 233], [207, 250, 254]),
        Green => ([22, 163, 74], [220, 252, 231]),
        Purple => ([139, 92, 246], [237, 233, 254]),
        Amber => ([245, 158, 11], [254, 243, 199]),
        Rose => ([244, 63, 94], [255, 228, 230]),
        Slate => ([100, 116, 139], [226, 232, 240]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> DockAgentConfig {
        DockAgentConfig {
            plugin_id: String::from("api-debugger"),
            plugin_name: String::from("API 调试器"),
            icon: String::from("icons/api.svg"),
            accent: PluginAccent::Blue,
        }
    }

    #[test]
    fn regular_arguments_do_not_enter_agent_mode() {
        let parsed = config_from_args([OsString::from("qingqi")]).unwrap();
        assert_eq!(parsed, None);
    }

    #[test]
    fn agent_configuration_round_trips_through_argument() {
        let expected = config();
        let argument = config_argument(&expected).unwrap();
        let parsed = config_from_args([
            OsString::from("qingqi"),
            OsString::from(DOCK_AGENT_ARG),
            OsString::from(argument),
        ])
        .unwrap();
        assert_eq!(parsed, Some(expected));
    }

    #[test]
    fn protocol_words_are_line_safe() {
        for word in [EVENT_READY, EVENT_ACTIVATE, EVENT_QUIT, COMMAND_SHUTDOWN] {
            assert!(!word.contains(['\n', '\r']));
        }
    }

    #[test]
    fn agent_events_are_parsed_from_lines() {
        assert_eq!(parse_agent_event("ready\n"), Some(DockAgentEvent::Ready));
        assert_eq!(
            parse_agent_event("activate\r\n"),
            Some(DockAgentEvent::Activate)
        );
        assert_eq!(parse_agent_event("quit"), Some(DockAgentEvent::Quit));
        assert_eq!(parse_agent_event("unknown"), None);
    }

    #[test]
    fn shutdown_and_parent_eof_request_exit() {
        assert!(parent_requests_shutdown(Some("shutdown\n")));
        assert!(parent_requests_shutdown(None));
        assert!(!parent_requests_shutdown(Some("unknown\n")));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn leases_keep_agent_entry_until_last_window_closes() {
        let expected = config();
        let plugin_id = expected.plugin_id.clone();
        let mut manager = DockAgentManager::default();
        manager.agents.insert(
            plugin_id.clone(),
            DockAgentEntry {
                config: expected,
                leases: 2,
                generation: 7,
                restart_attempts: 0,
                stopping: false,
                process: None,
            },
        );

        manager.release(&plugin_id);
        assert!(manager.is_current(&plugin_id, 7));
        assert_eq!(manager.agents[&plugin_id].leases, 1);

        manager.release(&plugin_id);
        assert!(!manager.agents.contains_key(&plugin_id));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn stale_generation_cannot_stop_replacement_agent() {
        let expected = config();
        let plugin_id = expected.plugin_id.clone();
        let mut manager = DockAgentManager::default();
        manager.agents.insert(
            plugin_id.clone(),
            DockAgentEntry {
                config: expected,
                leases: 1,
                generation: 9,
                restart_attempts: 0,
                stopping: false,
                process: None,
            },
        );

        assert!(!manager.mark_stopping(&plugin_id, 8));
        assert!(manager.is_current(&plugin_id, 9));
        assert!(manager.mark_stopping(&plugin_id, 9));
        assert!(!manager.is_current(&plugin_id, 9));
    }
}

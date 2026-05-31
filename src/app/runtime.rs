use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::Result;
use gpui::{App, Menu, MenuItem};

use crate::{
    app::{
        app_catalog::AppCatalog,
        app_index::AppIndexService,
        background::BackgroundSupervisor,
        events::AppEventBus,
        text_input::TextInput,
        theme_store::ThemeStore,
        window_controller::{PluginOpenTrace, WindowController, WindowControllerHandle},
    },
    core::{
        command_usage::CommandUsageStore,
        database::DatabaseService,
        keymap::{OpenClipboard, OpenLauncher, Quit, register_in_app_bindings},
        plugin::PluginManager,
        shortcut::{ShortcutAction, ShortcutService},
        storage::AppPaths,
    },
    features::registry::register_builtin_plugins,
    platform::power::PowerManager,
};

pub fn run() -> Result<()> {
    let paths = AppPaths::resolve()?;
    let log_path = paths.log_file("qingqi.log");
    init_tracing(log_path.as_path());
    install_panic_hook(log_path.with_file_name("qingqi-crash.log"));

    tracing::debug!(
        data_dir = %paths.data_dir().display(),
        log_file = %paths.log_file("qingqi.log").display(),
        "qingqi starting"
    );

    let theme_store = Arc::new(Mutex::new(ThemeStore::new(paths.config("theme.json"))));
    let events = AppEventBus::new();
    let database = Arc::new(DatabaseService::new(paths.clone()));
    database.register_database(crate::core::database::DatabaseSpec::app(
        "command-usage",
        "command_usage.db",
    ))?;
    database.register_database(crate::core::database::DatabaseSpec::app(
        "app-launcher/index",
        "app_index.db",
    ))?;
    let app_index_service = Arc::new(AppIndexService::with_events(
        Arc::clone(&database),
        events.clone(),
    ));
    let app_catalog = Arc::new(AppCatalog::new(Arc::clone(&app_index_service)));
    let mut plugins = PluginManager::new(
        events.clone(),
        CommandUsageStore::new(Arc::clone(&database), "command-usage"),
    );

    let clipboard_service = register_builtin_plugins(
        &mut plugins,
        paths.clone(),
        Arc::clone(&theme_store),
        Arc::clone(&database),
        events.clone(),
        Arc::clone(&app_index_service),
    )?;

    let plugins = Arc::new(Mutex::new(plugins));
    let window_controller = Arc::new(Mutex::new(WindowController::new(
        Arc::clone(&plugins),
        Arc::clone(&app_catalog),
        Arc::clone(&clipboard_service),
        events.clone(),
    )));
    let power_manager = Arc::new(Mutex::new(PowerManager::load(paths.config("power.json"))));
    let app = gpui::Application::new().with_assets(crate::app::assets::ProjectAssets);
    let plugins_for_shutdown = Arc::clone(&plugins);
    app.on_reopen({
        let window_controller = Arc::clone(&window_controller);
        move |cx| WindowController::show_launcher(Arc::clone(&window_controller), cx)
    });
    let database_for_shutdown = Arc::clone(&database);
    app.run(move |cx| {
        gpui_component::init(cx);
        TextInput::register_bindings(cx);

        cx.on_action({
            let window_controller = Arc::clone(&window_controller);
            move |_: &OpenLauncher, cx| {
                WindowController::show_launcher(Arc::clone(&window_controller), cx)
            }
        });
        cx.on_action({
            let window_controller = Arc::clone(&window_controller);
            move |_: &OpenClipboard, cx| {
                WindowController::open_plugin(Arc::clone(&window_controller), "clipboard", cx)
            }
        });
        cx.on_action(|_: &Quit, cx| cx.quit());

        #[cfg(target_os = "windows")]
        window_controller
            .lock()
            .unwrap_or_else(|e| {
                tracing::error!("window controller poisoned, recovering");
                e.into_inner()
            })
            .ensure_keep_alive_window(cx);

        set_menus(cx);
        app_catalog.start_background();
        crate::core::lock_or_recover(&plugins, "plugin-manager")
            .start_background(cx);
        let mut background = BackgroundSupervisor::new();
        background.start_theme_poll(Arc::clone(&theme_store), cx);

        register_in_app_bindings(cx);
        cx.on_action({
            let window_controller = Arc::clone(&window_controller);
            move |action: &ShortcutAction, cx| {
                let target = cx
                    .try_global::<ShortcutService>()
                    .and_then(|service| service.dispatch_app_action(action));
                if let Some(target) = target {
                    crate::core::shortcut::dispatch_target(
                        &target,
                        Arc::clone(&window_controller),
                        cx,
                    );
                }
            }
        });
        let mut shortcut_service = ShortcutService::new(Arc::clone(&plugins));
        if let Err(error) = shortcut_service.reload_from_plugins(cx) {
            tracing::warn!(error = %error, "shortcut registration failed");
        }
        background.start_hotkey_events(Arc::clone(&window_controller), cx);

        // Install the low-level keyboard hook with the entries resolved above
        // (e.g. Alt+Space).  We pass the entries explicitly rather than reading
        // them from `cx.try_global::<ShortcutService>()` — the service is not
        // registered as a global until further below, so reading it here would
        // silently skip the hook and break Alt+Space.
        #[cfg(target_os = "windows")]
        background.start_low_level_hook(
            shortcut_service.low_level_entries().to_vec(),
            Arc::clone(&window_controller),
            cx,
        );

        let initial_mode = crate::core::lock_or_recover(&power_manager, "power-manager").mode();
        match crate::platform::tray::install_tray(initial_mode) {
            Ok(()) => {
                background.start_tray_poll(
                    Arc::clone(&window_controller),
                    Arc::clone(&power_manager),
                    cx,
                );
                background.start_power_poll(Arc::clone(&power_manager), cx);
            }
            Err(error) => tracing::warn!(error, "system tray install failed"),
        }
        cx.set_global(shortcut_service);
        cx.set_global(background);
    });
    crate::core::lock_or_recover(&plugins_for_shutdown, "plugin-manager")
        .shutdown();
    database_for_shutdown.shutdown();
    Ok(())
}

/// Install a panic hook that records the panic through `tracing` (so it lands
/// in `qingqi.log`) and also appends it to a dedicated crash file.  Rust's
/// default hook only prints to stderr, which is invisible for a GUI process
/// launched without a console — so crashes ("闪退") would otherwise leave no
/// trace.  The hook chains to the previously-installed hook to preserve
/// stderr output.
fn install_panic_hook(crash_log: PathBuf) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let location = info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
            .unwrap_or_else(|| String::from("<unknown location>"));
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("<non-string panic payload>");
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>");

        // Route through tracing so the panic lands in qingqi.log (and stderr).
        tracing::error!(
            target: "qingqi::panic",
            thread = thread_name,
            location = %location,
            "panic: {message}\n{backtrace}"
        );

        // Belt-and-suspenders: append to a dedicated crash file in case the
        // tracing pipeline is itself unwinding when the panic fires.
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&crash_log) {
            let _ = writeln!(
                file,
                "==== panic ====\nthread : {thread_name}\nat     : {location}\nmessage: {message}\n{backtrace}\n"
            );
        }

        // Preserve the previously-installed behaviour (prints to stderr).
        default_hook(info);
    }));
}

fn init_tracing(log_path: &Path) {
    let log_file = open_log_file(log_path);
    let writer = move || TeeWriter {
        stderr: io::stderr(),
        file: log_file
            .as_ref()
            .and_then(|file| file.try_clone().ok())
            .map(Mutex::new),
    };

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("qingqi=debug,warn"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_writer(writer)
        .compact()
        .init();
}

fn open_log_file(path: &Path) -> Option<fs::File> {
    if let Some(parent) = path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        eprintln!("failed to create log dir {}: {error}", parent.display());
        return None;
    }

    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| {
            eprintln!("failed to open log file {}: {error}", path.display());
            error
        })
        .ok()
}

struct TeeWriter {
    stderr: io::Stderr,
    file: Option<Mutex<fs::File>>,
}

impl Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stderr.write_all(buf)?;
        if let Some(file) = &self.file
            && let Ok(mut file) = file.lock()
        {
            let _ = file.write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stderr.flush()?;
        if let Some(file) = &self.file
            && let Ok(mut file) = file.lock()
        {
            let _ = file.flush();
        }
        Ok(())
    }
}

pub fn run_command(
    window_controller: WindowControllerHandle,
    activation: crate::core::command::Activation,
    cx: &mut App,
) -> Option<String> {
    WindowController::run_command(window_controller, activation, cx)
}

pub fn run_command_with_trace(
    window_controller: WindowControllerHandle,
    activation: crate::core::command::Activation,
    cx: &mut App,
    trace: Option<PluginOpenTrace>,
) -> Option<String> {
    WindowController::run_command_with_trace(window_controller, activation, cx, trace)
}

fn set_menus(cx: &mut App) {
    cx.set_menus(vec![Menu {
        name: "Qingqi".into(),
        items: vec![
            MenuItem::action("打开启动器", OpenLauncher),
            MenuItem::action("剪贴板历史", OpenClipboard),
            MenuItem::separator(),
            MenuItem::action("退出", Quit),
        ],
    }]);
}

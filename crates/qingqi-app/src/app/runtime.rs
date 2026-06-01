use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::Result;
use gpui::{App, Menu, MenuItem};
use qingqi_plugin::host::{AppIndexHandleRef, ShortcutHandleRef, ThemeHandleRef};
use qingqi_plugin::{
    database::{DatabaseService, DatabaseSpec},
    events::AppEventBus,
    storage::AppPaths,
};

use crate::{
    app::{
        app_catalog::AppCatalog,
        app_index::AppIndexService,
        background::BackgroundSupervisor,
        theme_store::ThemeStore,
        window_controller::{PluginOpenTrace, WindowController, WindowControllerHandle},
    },
    core::{
        keymap::{OpenClipboard, OpenLauncher, Quit, register_in_app_bindings},
        shortcut::{ShortcutAction, ShortcutGlobal, ShortcutService},
    },
};
use qingqi_core::{command_usage::CommandUsageStore, plugin::PluginManager, registry::BuildCx};
use qingqi_platform::power::PowerManager;

struct ThemeHandleAdapter {
    store: Arc<Mutex<ThemeStore>>,
}

struct AppIndexHandleAdapter {
    service: Arc<AppIndexService>,
}

struct ShortcutHandleAdapter {
    service: Arc<Mutex<ShortcutService>>,
}

impl qingqi_plugin::host::ThemeHandle for ThemeHandleAdapter {
    fn mode(&self) -> qingqi_plugin::theme::ThemeMode {
        self.store
            .lock()
            .map(|store| store.mode())
            .unwrap_or(qingqi_plugin::theme::ThemeMode::System)
    }

    fn config_path(&self) -> String {
        self.store
            .lock()
            .map(|store| store.config_path().display().to_string())
            .unwrap_or_default()
    }

    fn system_dark(&self) -> bool {
        self.store
            .lock()
            .map(|store| store.system_dark())
            .unwrap_or(false)
    }

    fn set_mode(&self, mode: qingqi_plugin::theme::ThemeMode) -> anyhow::Result<()> {
        self.store
            .lock()
            .map_err(|_| anyhow::anyhow!("theme store lock poisoned"))?
            .set_mode(mode)
    }
}

impl qingqi_plugin::host::AppIndexHandle for AppIndexHandleAdapter {
    fn snapshot(&self) -> qingqi_plugin::app::AppIndexSnapshot {
        self.service.snapshot()
    }

    fn request_scan(&self) -> bool {
        Arc::clone(&self.service).request_scan()
    }
}

impl qingqi_plugin::host::ShortcutHandle for ShortcutHandleAdapter {
    fn views(&self) -> Vec<qingqi_plugin::shortcut::ShortcutView> {
        self.service
            .lock()
            .map(|service| service.views())
            .unwrap_or_default()
    }

    fn set_shortcut(
        &self,
        shortcut_id: &str,
        accelerator: &str,
        enabled: bool,
        cx: &mut App,
    ) -> anyhow::Result<()> {
        self.service
            .lock()
            .map_err(|_| anyhow::anyhow!("shortcut service lock poisoned"))?
            .set_shortcut(shortcut_id, accelerator, enabled, cx)
    }

    fn restore_shortcut(&self, shortcut_id: &str, cx: &mut App) -> anyhow::Result<()> {
        self.service
            .lock()
            .map_err(|_| anyhow::anyhow!("shortcut service lock poisoned"))?
            .restore_shortcut(shortcut_id, cx)
    }
}

pub struct AppHost {
    pub plugins: PluginManager,
    pub build_cx: BuildCx,
    pub theme_store: Arc<Mutex<ThemeStore>>,
    pub app_index_service: Arc<AppIndexService>,
    pub app_catalog: Arc<AppCatalog>,
    pub power_manager: Arc<Mutex<PowerManager>>,
    pub shortcut_service: Arc<Mutex<ShortcutService>>,
}

pub fn bootstrap() -> Result<AppHost> {
    let paths = AppPaths::resolve()?;
    let log_path = paths.log_file("qingqi.log");
    init_tracing(log_path.as_path());
    install_panic_hook(log_path.with_file_name("qingqi-crash.log"));

    tracing::debug!(
        data_dir = %paths.data_dir().display(),
        log_file = %paths.log_file("qingqi.log").display(),
        "qingqi starting"
    );

    let events = AppEventBus::new();
    let database = Arc::new(DatabaseService::new(paths.clone()));
    database.register_database(DatabaseSpec::app("command-usage", "command_usage.db"))?;
    database.register_database(DatabaseSpec::app("app-launcher/index", "app_index.db"))?;
    let theme_store = Arc::new(Mutex::new(ThemeStore::new(paths.config("theme.json"))));
    let app_index_service = Arc::new(AppIndexService::with_events(
        Arc::clone(&database),
        events.clone(),
    ));
    let app_catalog = Arc::new(AppCatalog::new(Arc::clone(&app_index_service)));
    let plugins = PluginManager::new(
        events.clone(),
        CommandUsageStore::new(Arc::clone(&database), "command-usage"),
    );
    let shortcut_service = Arc::new(Mutex::new(ShortcutService::default()));
    let build_cx = BuildCx::new(Arc::clone(&database), paths.clone(), events);
    let power_manager = Arc::new(Mutex::new(PowerManager::load(paths.config("power.json"))));

    Ok(AppHost {
        plugins,
        build_cx,
        theme_store,
        app_index_service,
        app_catalog,
        power_manager,
        shortcut_service,
    })
}

pub fn run(host: AppHost) -> Result<()> {
    let AppHost {
        plugins,
        build_cx,
        theme_store,
        app_index_service: _app_index_service,
        app_catalog,
        power_manager,
        shortcut_service,
    } = host;
    let plugins = Arc::new(Mutex::new(plugins));
    {
        let mut service = qingqi_core::lock_or_recover(&shortcut_service, "shortcut-service");
        *service = ShortcutService::new(Arc::clone(&plugins));
    }
    let window_controller = Arc::new(Mutex::new(WindowController::new(
        Arc::clone(&plugins),
        Arc::clone(&app_catalog),
        build_cx.events.clone(),
    )));
    let app = gpui::Application::new().with_assets(qingqi_ui::assets::ProjectAssets);
    let plugins_for_shutdown = Arc::clone(&plugins);
    app.on_reopen({
        let window_controller = Arc::clone(&window_controller);
        move |cx| WindowController::show_launcher(Arc::clone(&window_controller), cx)
    });
    let database_for_shutdown = Arc::clone(&build_cx.database);
    app.run(move |cx| {
        gpui_component::init(cx);
        qingqi_ui::text_input::TextInput::register_bindings(cx);

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
        lock_or_recover(&window_controller, "window_controller")
            .ensure_keep_alive_window(cx);

        set_menus(cx);
        app_catalog.start_background();
        qingqi_core::lock_or_recover(&plugins, "plugin-manager").start_background(cx);
        let mut background = BackgroundSupervisor::new();
        background.start_theme_poll(Arc::clone(&theme_store), cx);

        register_in_app_bindings(cx);
        cx.on_action({
            let window_controller = Arc::clone(&window_controller);
            move |action: &ShortcutAction, cx| {
                let target = cx
                    .try_global::<ShortcutGlobal>()
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
        {
            let mut service = qingqi_core::lock_or_recover(&shortcut_service, "shortcut-service");
            if let Err(error) = service.reload_from_plugins(cx) {
                tracing::warn!(error = %error, "shortcut registration failed");
            }
        }
        background.start_hotkey_events(Arc::clone(&window_controller), cx);

        // Install the low-level keyboard hook with the entries resolved above
        // (e.g. Alt+Space).  We pass the entries explicitly rather than reading
        // them from `cx.try_global::<ShortcutService>()` — the service is not
        // registered as a global until further below, so reading it here would
        // silently skip the hook and break Alt+Space.
        #[cfg(target_os = "windows")]
        background.start_low_level_hook(
            qingqi_core::lock_or_recover(&shortcut_service, "shortcut-service")
                .low_level_entries()
                .to_vec(),
            Arc::clone(&window_controller),
            cx,
        );

        let initial_mode = qingqi_core::lock_or_recover(&power_manager, "power-manager").mode();
        match qingqi_platform::tray::install_tray(initial_mode) {
            Ok(()) => {
                background.start_tray_events(
                    Arc::clone(&window_controller),
                    Arc::clone(&power_manager),
                    cx,
                );
                background.start_power_poll(Arc::clone(&power_manager), cx);
            }
            Err(error) => tracing::warn!(error, "system tray install failed"),
        }
        cx.set_global(ShortcutGlobal::new(Arc::clone(&shortcut_service)));
        cx.set_global(background);
    });
    qingqi_core::lock_or_recover(&plugins_for_shutdown, "plugin-manager").shutdown();
    database_for_shutdown.shutdown();
    Ok(())
}

pub fn theme_handle_ref(host: &AppHost) -> ThemeHandleRef {
    Arc::new(ThemeHandleAdapter {
        store: Arc::clone(&host.theme_store),
    })
}

pub fn app_index_handle_ref(host: &AppHost) -> AppIndexHandleRef {
    Arc::new(AppIndexHandleAdapter {
        service: Arc::clone(&host.app_index_service),
    })
}

pub fn shortcut_handle_ref(host: &AppHost) -> ShortcutHandleRef {
    Arc::new(ShortcutHandleAdapter {
        service: Arc::clone(&host.shortcut_service),
    })
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
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_log)
        {
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
    activation: qingqi_plugin::command::Activation,
    cx: &mut App,
) -> Option<String> {
    WindowController::run_command(window_controller, activation, cx)
}

pub fn run_command_with_trace(
    window_controller: WindowControllerHandle,
    activation: qingqi_plugin::command::Activation,
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

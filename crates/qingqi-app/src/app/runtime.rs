use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
};

use time::{Date, OffsetDateTime, format_description::FormatItem, macros::format_description};
use tracing_subscriber::Layer;
use tracing_subscriber::fmt::time::LocalTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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
        theme_service::ThemeService,
        theme_store::ThemeStore,
        tray_manager::{
            NetworkSpeedProvider, TrayManager, TrayManagerHandle, load_current_tray_settings,
        },
        window_controller::{PluginOpenTrace, WindowController, WindowControllerHandle},
    },
    core::{
        keymap::{OpenClipboard, OpenLauncher, Quit, register_in_app_bindings},
        shortcut::{ShortcutAction, ShortcutGlobal, ShortcutService},
    },
};
use qingqi_core::{
    command_catalog::{COMMAND_CATALOG_KEY, CommandCatalogStore},
    command_usage::CommandUsageStore,
    plugin::PluginManager,
    registry::BuildCx,
};
use qingqi_platform::power::PowerManager;

struct ThemeHandleAdapter {
    store: Arc<RwLock<ThemeStore>>,
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
            .read()
            .map(|store| store.mode())
            .unwrap_or(qingqi_plugin::theme::ThemeMode::System)
    }

    fn theme_name(&self) -> String {
        self.store
            .read()
            .map(|s| s.theme().to_string())
            .unwrap_or_default()
    }

    fn config_path(&self) -> String {
        self.store
            .read()
            .map(|store| store.config_path().display().to_string())
            .unwrap_or_default()
    }

    fn system_dark(&self) -> bool {
        self.store
            .read()
            .map(|store| store.system_dark())
            .unwrap_or(false)
    }

    fn set_mode(&self, mode: qingqi_plugin::theme::ThemeMode) -> anyhow::Result<()> {
        self.store
            .write()
            .map_err(|_| anyhow::anyhow!("theme store lock poisoned"))?
            .set_mode(mode)
    }

    fn apply_current(&self, cx: &mut gpui::App) -> anyhow::Result<()> {
        let store = self
            .store
            .read()
            .map_err(|_| anyhow::anyhow!("theme store lock poisoned"))?;
        let theme_name = store.theme().to_string();
        let mode = store.mode();
        drop(store);
        crate::app::theme_service::ThemeService::apply_theme(&theme_name, mode, cx);
        Ok(())
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
    pub theme_store: Arc<RwLock<ThemeStore>>,
    pub app_index_service: Arc<AppIndexService>,
    pub app_catalog: Arc<AppCatalog>,
    pub power_manager: Arc<Mutex<PowerManager>>,
    pub shortcut_service: Arc<Mutex<ShortcutService>>,
    pub paths: AppPaths,
    _log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

pub fn bootstrap() -> Result<AppHost> {
    let paths = AppPaths::resolve()?;

    let logs_dir = paths.data_dir().join("logs");
    let _log_guard = init_tracing(&logs_dir);
    install_panic_hook(logs_dir.join("qingqi-crash.log"));

    tracing::info!(
        data_dir = %paths.data_dir().display(),
        logs_dir = %logs_dir.display(),
        "qingqi starting"
    );

    let events = AppEventBus::new();
    let database = Arc::new(DatabaseService::new(paths.clone()));
    database.register_database(DatabaseSpec::app("command-usage", "command_usage.db"))?;
    database.register_database(DatabaseSpec::app(COMMAND_CATALOG_KEY, "command_catalog.db"))?;
    database.register_database(DatabaseSpec::app("app-launcher/index", "app_index.db"))?;
    let theme_store = Arc::new(RwLock::new(ThemeStore::new(paths.config("theme.json"))));
    let command_usage_store = CommandUsageStore::new(Arc::clone(&database), "command-usage");
    let app_index_service = Arc::new(AppIndexService::with_events(
        Arc::clone(&database),
        command_usage_store.clone(),
        events.clone(),
    ));
    let app_catalog = Arc::new(AppCatalog::new(Arc::clone(&app_index_service)));
    let plugins = PluginManager::new(
        events.clone(),
        command_usage_store,
        CommandCatalogStore::new(Arc::clone(&database), COMMAND_CATALOG_KEY),
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
        paths,
        _log_guard,
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
        paths,
        _log_guard: _,
    } = host;
    let plugins = Arc::new(Mutex::new(plugins));
    {
        let mut service = qingqi_core::lock_or_recover(&shortcut_service, "shortcut-service");
        *service = ShortcutService::new(Arc::clone(&plugins), Some(paths.config("shortcuts.json")));
    }
    let window_controller = Arc::new(Mutex::new(WindowController::new(
        Arc::clone(&plugins),
        Arc::clone(&app_catalog),
        build_cx.events.clone(),
    )));
    let tray_manager = TrayManagerHandle::new(TrayManager::new());
    let app = gpui::Application::new().with_assets(qingqi_ui::assets::ProjectAssets);
    let plugins_for_shutdown = Arc::clone(&plugins);
    app.on_reopen({
        let window_controller = Arc::clone(&window_controller);
        move |cx| WindowController::show_launcher(Arc::clone(&window_controller), cx)
    });
    let database_for_shutdown = Arc::clone(&build_cx.database);
    app.run(move |cx| {
        qingqi_platform::macos::hide_dock_icon();

        gpui_component::init(cx);

        // 初始化主题服务
        let themes_dir = paths.data_dir().join("config").join("themes");
        let theme_service = ThemeService::new(themes_dir);
        if let Err(e) = theme_service.init(cx) {
            tracing::error!(error = %e, "failed to init theme service");
        }

        // 应用已保存的主题
        let (initial_theme, initial_mode) = theme_store
            .read()
            .map(|s| (s.theme().to_string(), s.mode()))
            .unwrap_or_else(|_| {
                (
                    crate::app::theme_store::default_theme_name(),
                    qingqi_plugin::theme::ThemeMode::default(),
                )
            });
        tracing::info!(initial_theme = %initial_theme, initial_mode = ?initial_mode, "applying initial theme from store");
        ThemeService::apply_theme(&initial_theme, initial_mode, cx);

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
        qingqi_core::lock_or_recover(&window_controller, "window_controller")
            .ensure_keep_alive_window(cx);

        set_menus(cx);
        app_catalog.start_background();
        qingqi_core::lock_or_recover(&plugins, "plugin-manager").start_background(cx);
        let mut background = BackgroundSupervisor::new();
        background.start_theme_listener(Arc::clone(&theme_store), cx);

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
                let initial_tray_settings = load_current_tray_settings(&paths);
                qingqi_core::lock_or_recover(&tray_manager.0, "tray-manager")
                    .register_provider(Box::new(NetworkSpeedProvider::new(
                        Default::default(),
                        initial_tray_settings,
                    )));
                background.start_tray_events(
                    Arc::clone(&window_controller),
                    tray_manager.clone(),
                    Arc::clone(&power_manager),
                    cx,
                );
                background.start_tray_providers_with_paths(
                    tray_manager.clone(),
                    Some(paths.clone()),
                    cx,
                );
                background.start_power_listener(Arc::clone(&power_manager), cx);
            }
            Err(error) => tracing::warn!(error, "system tray install failed"),
        }
        cx.set_global(ShortcutGlobal::new(Arc::clone(&shortcut_service)));
        cx.set_global(tray_manager.clone());
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

fn init_tracing(logs_dir: &Path) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    // 确保日志目录存在（在 subscriber 初始化之前）
    if let Err(error) = fs::create_dir_all(logs_dir) {
        eprintln!("[qingqi] 无法创建日志目录 {}: {error}", logs_dir.display());
    }

    let env_filter = resolve_log_filter();

    // 文件 layer：完整格式，含 target 和时间戳，按天轮转
    let file_appender = tracing_appender::rolling::daily(logs_dir, "qingqi");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_timer(log_timer())
        .with_target(true)
        .with_thread_ids(true)
        .with_writer(non_blocking)
        .with_filter(env_filter.clone());

    // stderr layer：精简格式，与文件 layer 共用 RUST_LOG 过滤
    let stderr_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_timer(log_timer())
        .with_target(true)
        .with_writer(io::stderr)
        .with_filter(env_filter);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .init();

    // 清理旧日志
    prune_old_logs(logs_dir, 7);

    Some(guard)
}

fn log_timer() -> LocalTime<&'static [FormatItem<'static>]> {
    LocalTime::new(format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"
    ))
}

/// 解析日志级别过滤：RUST_LOG 环境变量，未设置时使用编译时默认
fn resolve_log_filter() -> tracing_subscriber::EnvFilter {
    if let Ok(filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        return filter;
    }

    // 编译时默认
    if cfg!(debug_assertions) {
        tracing_subscriber::EnvFilter::new(
            "debug,qingqi_ssh=debug,russh=debug,russh_sftp=debug,suppaftp=debug",
        )
    } else {
        tracing_subscriber::EnvFilter::new("warn,qingqi=info")
    }
}

/// 清理超过 `retain_days` 天的旧日志文件
fn prune_old_logs(logs_dir: &Path, retain_days: u32) {
    let fmt = format_description!("[year]-[month]-[day]");
    let Some(cutoff) =
        OffsetDateTime::now_utc().checked_sub(time::Duration::days(retain_days as i64))
    else {
        return;
    };

    let Ok(entries) = fs::read_dir(logs_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // 匹配 tracing_appender rolling::daily 格式: qingqi.YYYY-MM-DD
        if !name.starts_with("qingqi.") {
            continue;
        }
        let date_str = &name["qingqi.".len()..];
        if let Ok(date) = Date::parse(date_str, &fmt) {
            if date < cutoff.date() {
                let _ = fs::remove_file(&path);
            }
        }
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

pub fn run_command_with_input(
    window_controller: WindowControllerHandle,
    activation: qingqi_plugin::command::Activation,
    cx: &mut App,
    launch_input: Option<String>,
) -> Option<String> {
    WindowController::run_command_with_input(window_controller, activation, cx, launch_input)
}

pub fn run_command_with_input_with_trace(
    window_controller: WindowControllerHandle,
    activation: qingqi_plugin::command::Activation,
    cx: &mut App,
    trace: Option<PluginOpenTrace>,
    launch_input: Option<String>,
) -> Option<String> {
    WindowController::run_command_with_input_with_trace(
        window_controller,
        activation,
        cx,
        trace,
        launch_input,
    )
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

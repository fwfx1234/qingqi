use std::{
    cell::RefCell,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    rc::Rc,
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
    features::{
        clipboard::{plugin::ClipboardRuntime, service::ClipboardService},
        registry::register_builtin_plugins,
    },
    platform::power::PowerManager,
};

pub fn run() -> Result<()> {
    let paths = AppPaths::resolve()?;
    init_tracing(paths.log_file("qingqi.log").as_path());

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
    database.register_database(crate::core::database::DatabaseSpec::feature(
        "clipboard",
        "history",
        "clipboard.db",
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

    let clipboard_runtime = ClipboardRuntime::new(ClipboardService::new(
        Arc::clone(&database),
        paths.database("clipboard.db"),
    ));
    let clipboard_service = clipboard_runtime.service();
    clipboard_runtime
        .service()
        .lock()
        .map(|mut service| service.start())
        .ok();
    plugins.register(Box::new(clipboard_runtime));
    register_builtin_plugins(
        &mut plugins,
        paths.clone(),
        Arc::clone(&theme_store),
        Arc::clone(&database),
        events.clone(),
        Arc::clone(&app_index_service),
    )?;

    let plugins = Rc::new(RefCell::new(plugins));
    let window_controller = Rc::new(RefCell::new(WindowController::new(
        Rc::clone(&plugins),
        Arc::clone(&app_catalog),
        Arc::clone(&clipboard_service),
        events.clone(),
    )));
    let power_manager = Rc::new(RefCell::new(PowerManager::load(paths.config("power.json"))));
    let app = gpui::Application::new().with_assets(crate::app::assets::ProjectAssets);
    let plugins_for_shutdown = Rc::clone(&plugins);
    app.on_reopen({
        let window_controller = Rc::clone(&window_controller);
        move |cx| WindowController::show_launcher(Rc::clone(&window_controller), cx)
    });
    let database_for_shutdown = Arc::clone(&database);
    app.run(move |cx| {
        gpui_component::init(cx);
        TextInput::register_bindings(cx);

        cx.on_action({
            let window_controller = Rc::clone(&window_controller);
            move |_: &OpenLauncher, cx| {
                WindowController::show_launcher(Rc::clone(&window_controller), cx)
            }
        });
        cx.on_action({
            let window_controller = Rc::clone(&window_controller);
            move |_: &OpenClipboard, cx| {
                WindowController::open_plugin(Rc::clone(&window_controller), "clipboard", cx)
            }
        });
        cx.on_action(|_: &Quit, cx| cx.quit());

        set_menus(cx);
        app_catalog.start_background();
        plugins.borrow_mut().start_background(cx);
        let mut background = BackgroundSupervisor::new();
        background.start_theme_poll(Arc::clone(&theme_store), cx);

        register_in_app_bindings(cx);
        cx.on_action({
            let window_controller = Rc::clone(&window_controller);
            move |action: &ShortcutAction, cx| {
                let target = cx
                    .try_global::<ShortcutService>()
                    .and_then(|service| service.dispatch_app_action(action));
                if let Some(target) = target {
                    crate::core::shortcut::dispatch_target(
                        &target,
                        Rc::clone(&window_controller),
                        cx,
                    );
                }
            }
        });
        let mut shortcut_service = ShortcutService::new(Rc::clone(&plugins));
        if let Err(error) = shortcut_service.reload_from_plugins(cx) {
            tracing::warn!(error = %error, "shortcut registration failed");
        }
        background.start_hotkey_events(Rc::clone(&window_controller), cx);

        let initial_mode = power_manager.borrow().mode();
        match crate::platform::tray::install_tray(initial_mode) {
            Ok(()) => {
                background.start_tray_poll(
                    Rc::clone(&window_controller),
                    Rc::clone(&power_manager),
                    cx,
                );
                background.start_power_poll(Rc::clone(&power_manager), cx);
            }
            Err(error) => tracing::warn!(error, "system tray install failed"),
        }
        cx.set_global(shortcut_service);
        cx.set_global(background);
    });
    plugins_for_shutdown.borrow_mut().shutdown();
    database_for_shutdown.shutdown();
    Ok(())
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

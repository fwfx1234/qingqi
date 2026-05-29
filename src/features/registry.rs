use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::{
    app::theme_store::ThemeStore,
    core::{
        database::{DatabaseService, DatabaseSpec},
        plugin::{PluginManager, PluginRuntime},
        storage::AppPaths,
    },
    features::{
        about::plugin as about_plugin,
        anti_peeping::plugin::AntiPeepingRuntime,
        api_debugger::plugin::ApiDebuggerRuntime,
        app_launcher::{plugin::AppLauncherRuntime, service::AppIndexService},
        download_manager::plugin::DownloadManagerRuntime,
        ftp_sftp_ssh_client::plugin::FtpSftpSshRuntime,
        gpui_demo::plugin::GpuiDemoRuntime,
        http_capture::plugin::HttpCaptureRuntime,
        image_compress::plugin as image_compress_plugin,
        json_parser::plugin as json_parser_plugin,
        qr_code::plugin as qr_code_plugin,
        quick_launch::plugin::QuickLaunchRuntime,
        system_settings::{plugin::SystemSettingsRuntime, settings_store::SettingsStore},
    },
};

fn register_runtime(
    plugins: &mut PluginManager,
    database: &Arc<DatabaseService>,
    runtime: Box<dyn PluginRuntime>,
) -> Result<()> {
    let specs = runtime.database_specs();
    if !specs.is_empty() {
        database.register_databases(specs)?;
    }
    plugins.register(runtime);
    Ok(())
}

pub fn register_builtin_plugins(
    plugins: &mut PluginManager,
    paths: AppPaths,
    theme_store: Arc<Mutex<ThemeStore>>,
    database: Arc<DatabaseService>,
) -> Result<()> {
    database.register_database(DatabaseSpec::app("command-usage", "command_usage.db"))?;
    database.register_database(DatabaseSpec::app("app-launcher/index", "app_index.db"))?;

    let app_index_service = Arc::new(AppIndexService::new(Arc::clone(&database)));
    let settings_store = Arc::new(Mutex::new(SettingsStore::new(
        paths.config("system_settings.json"),
    )));

    register_runtime(plugins, &database, Box::new(about_plugin::runtime()))?;
    register_runtime(
        plugins,
        &database,
        Box::new(AntiPeepingRuntime::new(paths.clone())),
    )?;
    register_runtime(
        plugins,
        &database,
        Box::new(AppLauncherRuntime::with_service(Arc::clone(&app_index_service))),
    )?;
    register_runtime(
        plugins,
        &database,
        Box::new(ApiDebuggerRuntime::new(Arc::clone(&database), paths.clone())),
    )?;
    register_runtime(
        plugins,
        &database,
        Box::new(DownloadManagerRuntime::new(Arc::clone(&database), paths.clone())?),
    )?;
    register_runtime(
        plugins,
        &database,
        Box::new(image_compress_plugin::runtime(paths.clone())),
    )?;
    register_runtime(plugins, &database, Box::new(json_parser_plugin::runtime()))?;
    register_runtime(plugins, &database, Box::new(qr_code_plugin::runtime(paths.clone())))?;
    database.register_databases(vec![DatabaseSpec::feature(
        "quick-launch",
        "actions",
        "actions.db",
    )])?;
    register_runtime(
        plugins,
        &database,
        Box::new(QuickLaunchRuntime::new(Arc::clone(&database), paths.clone())?),
    )?;
    register_runtime(
        plugins,
        &database,
        Box::new(SystemSettingsRuntime::new(
            Arc::clone(&theme_store),
            paths.clone(),
            settings_store,
            Some(app_index_service),
        )),
    )?;
    register_runtime(plugins, &database, Box::new(GpuiDemoRuntime::new()))?;
    register_runtime(
        plugins,
        &database,
        Box::new(FtpSftpSshRuntime::new(Arc::clone(&database), paths.clone())?),
    )?;
    database.register_databases(vec![DatabaseSpec::feature(
        "http-capture",
        "capture",
        "capture.db",
    )])?;
    register_runtime(
        plugins,
        &database,
        Box::new(HttpCaptureRuntime::new(Arc::clone(&database), paths)?),
    )?;

    Ok(())
}

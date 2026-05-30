use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::{
    app::{app_index::AppIndexService, events::AppEventBus, theme_store::ThemeStore},
    core::{
        database::{DatabaseService, DatabaseSpec},
        plugin::PluginManager,
        registry::{BuildCx, FeatureRegistry, PluginDescriptor},
        storage::AppPaths,
    },
    features::{
        about::{manifest as about_manifest, plugin::AboutPlugin},
        anti_peeping::{manifest as anti_peeping_manifest, plugin::AntiPeepingPlugin},
        api_debugger::{manifest as api_debugger_manifest, plugin::ApiDebuggerPlugin},
        clipboard::service::ClipboardService,
        download_manager::{manifest as download_manager_manifest, plugin::DownloadManagerPlugin},
        ftp_sftp_ssh_client::{manifest as ftp_sftp_ssh_manifest, plugin::FtpSftpSshPlugin},
        gpui_demo::plugin::GpuiDemoPlugin,
        http_capture::{manifest as http_capture_manifest, plugin::HttpCapturePlugin},
        image_compress::{manifest as image_compress_manifest, plugin as image_compress_plugin},
        json_parser::{manifest as json_parser_manifest, plugin as json_parser_plugin},
        qr_code::{manifest as qr_code_manifest, plugin as qr_code_plugin},
        quick_launch::{manifest as quick_launch_manifest, plugin::QuickLaunchPlugin},
        system_settings::{plugin::SystemSettingsPlugin, settings_store::SettingsStore},
    },
};

pub fn register_builtin_plugins(
    plugins: &mut PluginManager,
    paths: AppPaths,
    theme_store: Arc<Mutex<ThemeStore>>,
    database: Arc<DatabaseService>,
    events: AppEventBus,
    app_index_service: Arc<AppIndexService>,
) -> Result<Arc<Mutex<ClipboardService>>> {
    let settings_store = Arc::new(Mutex::new(SettingsStore::new(
        paths.config("system_settings.json"),
    )));
    let clipboard_service = Arc::new(Mutex::new(ClipboardService::new(
        Arc::clone(&database),
        paths.data_dir().join("clipboard.db"),
    )));

    let build_cx = BuildCx::new(Arc::clone(&database), paths.clone(), events);
    let mut registry = FeatureRegistry::new();

    registry.register(
        PluginDescriptor::builtin(about_manifest::manifest()),
        |_| Ok(Box::new(AboutPlugin)),
    );
    registry.register(
        PluginDescriptor::builtin(anti_peeping_manifest::manifest()),
        |cx| Ok(Box::new(AntiPeepingPlugin::new(cx.paths.clone()))),
    );
    registry.register(
        PluginDescriptor::builtin(api_debugger_manifest::manifest()).with_databases(vec![
            DatabaseSpec::app("api_debugger/main", "api_debugger.db"),
        ]),
        |cx| {
            Ok(Box::new(ApiDebuggerPlugin::new(
                Arc::clone(&cx.database),
                cx.paths.clone(),
            )))
        },
    );
    registry.register(
        PluginDescriptor::builtin(download_manager_manifest::manifest()).with_databases(vec![
            DatabaseSpec::feature("download-manager", "tasks", "tasks.db"),
        ]),
        |cx| {
            Ok(Box::new(DownloadManagerPlugin::new(
                Arc::clone(&cx.database),
                cx.paths.clone(),
            )?))
        },
    );
    registry.register(
        PluginDescriptor::builtin(image_compress_manifest::manifest()),
        |cx| Ok(Box::new(image_compress_plugin::runtime(cx.paths.clone()))),
    );
    registry.register(
        PluginDescriptor::builtin(json_parser_manifest::manifest()),
        |_| Ok(Box::new(json_parser_plugin::runtime())),
    );
    registry.register(
        PluginDescriptor::builtin(qr_code_manifest::manifest()),
        |cx| Ok(Box::new(qr_code_plugin::runtime(cx.paths.clone()))),
    );
    registry.register(
        PluginDescriptor::builtin(quick_launch_manifest::manifest()).with_databases(vec![
            DatabaseSpec::feature("quick-launch", "actions", "actions.db"),
        ]),
        |cx| {
            Ok(Box::new(QuickLaunchPlugin::new(
                Arc::clone(&cx.database),
                cx.paths.clone(),
            )?))
        },
    );
    registry.register(
        PluginDescriptor::builtin(SystemSettingsPlugin::manifest_static()),
        {
            let settings_store = Arc::clone(&settings_store);
            let app_index_service = Arc::clone(&app_index_service);
            let theme_store = Arc::clone(&theme_store);
            move |cx| {
                Ok(Box::new(SystemSettingsPlugin::new(
                    Arc::clone(&theme_store),
                    cx.paths.clone(),
                    settings_store,
                    Some(app_index_service),
                )))
            }
        },
    );
    registry.register(
        PluginDescriptor::builtin(GpuiDemoPlugin::manifest_static()),
        |_| Ok(Box::new(GpuiDemoPlugin::new())),
    );
    registry.register(
        PluginDescriptor::builtin(ftp_sftp_ssh_manifest::manifest()).with_databases(vec![
            DatabaseSpec::feature("ftp-sftp-ssh-client", "profiles", "profiles.db"),
        ]),
        |cx| {
            Ok(Box::new(FtpSftpSshPlugin::new(
                Arc::clone(&cx.database),
                cx.paths.clone(),
            )?))
        },
    );
    registry.register(
        PluginDescriptor::builtin(http_capture_manifest::manifest()).with_databases(vec![
            DatabaseSpec::feature("http-capture", "capture", "capture.db"),
        ]),
        |cx| {
            Ok(Box::new(HttpCapturePlugin::new(
                Arc::clone(&cx.database),
                cx.paths.clone(),
            )?))
        },
    );

    registry.build_all(&build_cx, plugins)?;
    Ok(clipboard_service)
}

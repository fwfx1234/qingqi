use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::{
    app::theme_store::ThemeStore,
    core::{plugin::PluginManager, storage::AppPaths},
    features::{
        about::plugin::AboutRuntime,
        api_debugger::plugin::ApiDebuggerRuntime,
        app_launcher::{plugin::AppLauncherRuntime, service::AppIndexService},
        download_manager::plugin::DownloadManagerRuntime,
        ftp_sftp_ssh_client::plugin::FtpSftpSshRuntime,
        gpui_demo::plugin::GpuiDemoRuntime,
        http_capture::plugin::HttpCaptureRuntime,
        image_compress::plugin::ImageCompressRuntime,
        json_parser::plugin::JsonParserRuntime,
        qr_code::plugin::QrCodeRuntime,
        quick_launch::plugin::QuickLaunchRuntime,
        system_settings::{plugin::SystemSettingsRuntime, settings_store::SettingsStore},
    },
};

pub fn register_builtin_plugins(
    plugins: &mut PluginManager,
    paths: AppPaths,
    theme_store: Arc<Mutex<ThemeStore>>,
) -> Result<()> {
    let app_index_service = Arc::new(AppIndexService::new(paths.clone()));
    let settings_store = Arc::new(Mutex::new(SettingsStore::new(
        paths.config("system_settings.json"),
    )));

    plugins.register(Box::new(AboutRuntime::new()));
    plugins.register(Box::new(AppLauncherRuntime::with_service(Arc::clone(
        &app_index_service,
    ))));
    plugins.register(Box::new(ApiDebuggerRuntime::new(paths.clone())));
    plugins.register(Box::new(DownloadManagerRuntime::new(paths.clone())?));
    plugins.register(Box::new(ImageCompressRuntime::new(paths.clone())));
    plugins.register(Box::new(JsonParserRuntime::new()));
    plugins.register(Box::new(QrCodeRuntime::new(paths.clone())));
    plugins.register(Box::new(QuickLaunchRuntime::new(paths.clone())?));
    plugins.register(Box::new(SystemSettingsRuntime::new(
        Arc::clone(&theme_store),
        paths.clone(),
        settings_store,
        Some(app_index_service),
    )));
    plugins.register(Box::new(GpuiDemoRuntime::new()));
    plugins.register(Box::new(FtpSftpSshRuntime::new(paths.clone())?));
    plugins.register(Box::new(HttpCaptureRuntime::new(paths)?));

    Ok(())
}

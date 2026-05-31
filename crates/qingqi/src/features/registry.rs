use std::sync::Arc;

use anyhow::Result;
use qingqi_app::app::runtime::{
    AppHost, app_index_handle_ref, shortcut_handle_ref, theme_handle_ref,
};
use qingqi_core::registry::{FeatureRegistry, PluginDescriptor};
use qingqi_feature_about as feature_about;
use qingqi_feature_anti_peeping as feature_anti_peeping;
use qingqi_feature_api_debugger as feature_api_debugger;
use qingqi_feature_clipboard as feature_clipboard;
use qingqi_feature_download_manager as feature_download_manager;
use qingqi_feature_ftp_sftp_ssh_client as feature_ftp_sftp_ssh_client;
use qingqi_feature_gpui_demo as feature_gpui_demo;
use qingqi_feature_http_capture as feature_http_capture;
use qingqi_feature_image_compress as feature_image_compress;
use qingqi_feature_json_parser as feature_json_parser;
use qingqi_feature_qr_code as feature_qr_code;
use qingqi_feature_quick_launch as feature_quick_launch;
use qingqi_feature_system_settings as feature_system_settings;
use qingqi_plugin::clipboard::ClipboardContext;

pub fn register_builtin_plugins(host: &mut AppHost) -> Result<Arc<dyn ClipboardContext>> {
    let theme_handle = theme_handle_ref(host);
    let app_index_handle = app_index_handle_ref(host);
    let shortcut_handle = shortcut_handle_ref(host);
    let (clipboard_plugin, clipboard_context) = feature_clipboard::build_shared(
        Arc::clone(&host.build_cx.database),
        host.build_cx.paths.clone(),
    )?;
    let mut registry = FeatureRegistry::new();

    registry.register(
        PluginDescriptor::builtin(feature_clipboard::manifest::manifest())
            .with_databases(feature_clipboard::databases()),
        move |_| Ok(clipboard_plugin),
    );
    registry.register(
        PluginDescriptor::builtin(feature_about::manifest::manifest()),
        |_| feature_about::build(),
    );
    registry.register(
        PluginDescriptor::builtin(feature_anti_peeping::manifest::manifest()),
        |cx| feature_anti_peeping::build(cx.paths.clone()),
    );
    registry.register(
        PluginDescriptor::builtin(feature_api_debugger::manifest::manifest())
            .with_databases(feature_api_debugger::databases()),
        |cx| feature_api_debugger::build(Arc::clone(&cx.database), cx.paths.clone()),
    );
    registry.register(
        PluginDescriptor::builtin(feature_download_manager::manifest::manifest())
            .with_databases(feature_download_manager::databases()),
        |cx| feature_download_manager::build(Arc::clone(&cx.database), cx.paths.clone()),
    );
    registry.register(
        PluginDescriptor::builtin(feature_image_compress::manifest::manifest())
            .with_databases(feature_image_compress::databases()),
        |cx| feature_image_compress::build(cx.paths.clone()),
    );
    registry.register(
        PluginDescriptor::builtin(feature_json_parser::manifest::manifest()),
        |_| feature_json_parser::build(),
    );
    registry.register(
        PluginDescriptor::builtin(feature_qr_code::manifest::manifest()),
        |cx| feature_qr_code::build(cx.paths.clone()),
    );
    registry.register(
        PluginDescriptor::builtin(feature_quick_launch::manifest::manifest())
            .with_databases(feature_quick_launch::databases()),
        |cx| feature_quick_launch::build(Arc::clone(&cx.database), cx.paths.clone()),
    );
    registry.register(
        PluginDescriptor::builtin(feature_system_settings::manifest::manifest())
        .with_databases(feature_system_settings::databases()),
        {
            let app_index_handle = Arc::clone(&app_index_handle);
            let shortcut_handle = Arc::clone(&shortcut_handle);
            let theme_handle = Arc::clone(&theme_handle);
            move |cx| {
                feature_system_settings::build(
                    Arc::clone(&theme_handle),
                    cx.paths.clone(),
                    Some(app_index_handle),
                    Some(shortcut_handle),
                )
            }
        },
    );
    registry.register(
        PluginDescriptor::builtin(feature_gpui_demo::manifest::manifest()),
        |_| feature_gpui_demo::build(),
    );
    registry.register(
        PluginDescriptor::builtin(feature_ftp_sftp_ssh_client::manifest::manifest())
            .with_databases(feature_ftp_sftp_ssh_client::databases()),
        |cx| feature_ftp_sftp_ssh_client::build(Arc::clone(&cx.database), cx.paths.clone()),
    );
    registry.register(
        PluginDescriptor::builtin(feature_http_capture::manifest::manifest())
            .with_databases(feature_http_capture::databases()),
        |cx| feature_http_capture::build(Arc::clone(&cx.database), cx.paths.clone()),
    );

    registry.build_all(&host.build_cx, &mut host.plugins)?;
    Ok(clipboard_context)
}

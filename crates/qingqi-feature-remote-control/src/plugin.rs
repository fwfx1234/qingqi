use std::sync::Arc;

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};
use qingqi_plugin::{
    command::Command,
    database::DatabaseService,
    plugin::{Manifest, Plugin, PluginCx, PluginId, PluginView, WindowView},
};

use crate::manifest::{self, PLUGIN_ID};
use crate::service::RemoteControlService;

pub struct RemoteControlPlugin {
    service: Arc<RemoteControlService>,
    database: Arc<DatabaseService>,
}

impl RemoteControlPlugin {
    pub fn new(database: Arc<DatabaseService>, paths: qingqi_plugin::storage::AppPaths) -> anyhow::Result<Self> {
        Ok(Self {
            service: Arc::new(RemoteControlService::new(paths)),
            database,
        })
    }

    pub fn service(&self) -> Arc<RemoteControlService> {
        Arc::clone(&self.service)
    }

    pub fn generate_pin(&mut self) -> String {
        self.service.generate_pin()
    }
}

impl Plugin for RemoteControlPlugin {
    fn manifest(&self) -> Manifest {
        manifest::manifest()
    }

    fn commands(&self, _query: &str) -> Vec<Command> {
        let m = self.manifest();
        vec![Command::plugin_open(
            m.id.as_ref(),
            m.name.as_ref(),
            m.description.as_ref(),
            m.keywords.iter().map(|s| s.as_ref()),
            m.prefixes.iter().map(|s| s.as_ref()),
            m.icon.as_str(),
        )]
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let view = cx.app.new(|cx| {
            let mut view = super::view::RemoteControlView::new(
                Arc::clone(&self.service),
                Arc::clone(&self.database),
            );
            view.init(cx);
            view
        });
        Ok(PluginView::Window(Box::new(RemoteControlWindow { view })))
    }

    fn start_background(&mut self, _cx: &mut PluginCx<'_>) {
        // Auto-start the remote control server on Windows
        #[cfg(target_os = "windows")]
        {
            tracing::info!("[远程控制] 插件后台启动，准备自动启动服务器...");

            // Update service state so the view shows "running" when opened
            self.service.set_server_running(true, 3721);

            let state = crate::server::AppState::new(
                (*self.service).clone(),
                Arc::clone(&self.database),
            );
            let port = 3721;
            tracing::info!("[远程控制] 正在后台启动服务器，端口: {}", port);

            qingqi_core::tokio_runtime::spawn(async move {
                match crate::server::RemoteServer::run(state, port).await {
                    Ok((addr, server_handle)) => {
                        tracing::info!("[远程控制] 服务器自动启动成功，监听地址: {}", addr);
                        let _ = server_handle.await;
                        tracing::warn!("[远程控制] 服务器任务已结束");
                    }
                    Err(e) => {
                        tracing::error!("[远程控制] 自动启动失败: {}", e);
                    }
                }
            });
        }

        #[cfg(not(target_os = "windows"))]
        {
            tracing::info!("[远程控制] 非 Windows 系统，跳过自动启动");
        }
    }

    fn shutdown(&mut self) {
        // Cleanup
    }
}

struct RemoteControlWindow {
    view: Entity<super::view::RemoteControlView>,
}

impl WindowView for RemoteControlWindow {
    fn plugin_id(&self) -> PluginId {
        PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "远程控制".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.view.clone().into_any_element()
    }

    fn on_close(&mut self) {}
}

use std::{sync::Arc, time::Duration};

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use crate::{manifest, service::FtpSftpSshService, view};
use qingqi_plugin::{
    database::DatabaseService,
    events::{AppEventBus, AppEventKind},
    plugin::{Manifest, Plugin, PluginCx, PluginId, PluginView, WindowView},
    storage::AppPaths,
};

pub struct FtpSftpSshPlugin {
    database: Arc<DatabaseService>,
    paths: AppPaths,
    service: Option<Arc<FtpSftpSshService>>,
    watch_started: bool,
}

impl FtpSftpSshPlugin {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> anyhow::Result<Self> {
        Ok(Self {
            database,
            paths,
            service: None,
            watch_started: false,
        })
    }

    fn service(&mut self) -> anyhow::Result<Arc<FtpSftpSshService>> {
        if let Some(service) = &self.service {
            return Ok(Arc::clone(service));
        }
        let service = Arc::new(FtpSftpSshService::new(
            Arc::clone(&self.database),
            self.paths.clone(),
        )?);
        self.service = Some(Arc::clone(&service));
        Ok(service)
    }

    fn ensure_watcher(
        &mut self,
        service: Arc<FtpSftpSshService>,
        events: AppEventBus,
        cx: &mut App,
    ) {
        if self.watch_started {
            return;
        }
        self.watch_started = true;

        cx.spawn(async move |async_cx| {
            let mut revision = service.revision();
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                if service.has_live_terminal() {
                    let _ = service.active_terminal_snapshot();
                    let _ = service.active_protocol_log();
                }
                let next_revision = service.revision();
                if next_revision != revision {
                    revision = next_revision;
                    events.publish(manifest::PLUGIN_ID, AppEventKind::FeatureChanged);
                }
            }
        })
        .detach();
    }
}

impl Plugin for FtpSftpSshPlugin {
    fn manifest(&self) -> Manifest {
        manifest::manifest()
    }
    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let service = self.service()?;
        self.ensure_watcher(Arc::clone(&service), cx.events.clone(), cx.app);
        let handle = cx.app.new(|_cx| view::FtpSftpSshView::new(service));
        Ok(PluginView::Window(Box::new(FtpSftpSshView {
            panel: handle,
        })))
    }

    fn close_idle(&mut self) {
        if !self.watch_started {
            self.service = None;
        }
    }

    fn shutdown(&mut self) {
        if let Some(service) = &self.service {
            service.shutdown();
        }
    }
}

struct FtpSftpSshView {
    panel: Entity<view::FtpSftpSshView>,
}

impl WindowView for FtpSftpSshView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "FTP/SFTP/SSH 客户端".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.panel.clone().into_any_element()
    }
}

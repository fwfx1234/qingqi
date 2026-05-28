use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AnyElement, App, IntoElement, Window};

use crate::{
    app::events::{AppEventBus, AppEventKind},
    core::{
        plugin::{PluginManifest, PluginRuntime, PluginSession},
        storage::AppPaths,
    },
    features::ftp_sftp_ssh_client::{manifest, service::FtpSftpSshService, view},
};

pub struct FtpSftpSshRuntime {
    service: Arc<FtpSftpSshService>,
    watch_started: bool,
}

impl FtpSftpSshRuntime {
    pub fn new(paths: AppPaths) -> anyhow::Result<Self> {
        Ok(Self {
            service: Arc::new(FtpSftpSshService::new(paths)?),
            watch_started: false,
        })
    }

    fn ensure_watcher(&mut self, events: AppEventBus, cx: &mut App) {
        if self.watch_started {
            return;
        }
        self.watch_started = true;

        let service = Arc::clone(&self.service);
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

impl PluginRuntime for FtpSftpSshRuntime {
    fn manifest(&self) -> PluginManifest {
        manifest::manifest()
    }

    fn open_session(
        &mut self,
        events: AppEventBus,
        cx: &mut App,
    ) -> anyhow::Result<Box<dyn PluginSession>> {
        self.ensure_watcher(events, cx);
        Ok(Box::new(FtpSftpSshSession {
            panel: Rc::new(RefCell::new(view::FtpSftpSshPanel::new(Arc::clone(
                &self.service,
            )))),
        }))
    }

    fn close_idle(&mut self) {}

    fn shutdown(&mut self) {
        self.service.shutdown();
    }
}

struct FtpSftpSshSession {
    panel: Rc<RefCell<view::FtpSftpSshPanel>>,
}

impl PluginSession for FtpSftpSshSession {
    fn plugin_id(&self) -> &'static str {
        manifest::PLUGIN_ID
    }

    fn title(&self) -> &'static str {
        "FTP/SFTP/SSH 客户端"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        view::FtpSftpSshElement {
            panel: Rc::clone(&self.panel),
        }
        .into_any_element()
    }
}

use std::sync::Arc;

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use crate::{manifest, runtime::RemoteRuntime, view};
use qingqi_plugin::{
    database::DatabaseService,
    plugin::{Manifest, Plugin, PluginCx, PluginId, PluginView, WindowView},
    storage::AppPaths,
};

pub struct FtpSftpSshPlugin {
    database: Arc<DatabaseService>,
    paths: AppPaths,
    runtime: Option<Arc<RemoteRuntime>>,
}

impl FtpSftpSshPlugin {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> anyhow::Result<Self> {
        Ok(Self {
            database,
            paths,
            runtime: None,
        })
    }

    fn runtime(&mut self) -> anyhow::Result<Arc<RemoteRuntime>> {
        if let Some(runtime) = &self.runtime {
            return Ok(Arc::clone(runtime));
        }
        let runtime = Arc::new(RemoteRuntime::new(
            Arc::clone(&self.database),
            self.paths.clone(),
        )?);
        self.runtime = Some(Arc::clone(&runtime));
        Ok(runtime)
    }
}

impl Plugin for FtpSftpSshPlugin {
    fn manifest(&self) -> Manifest {
        manifest::manifest()
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let runtime = self.runtime()?;
        let panel = cx.app.new(|cx| view::FtpSftpSshView::new(runtime, cx));
        Ok(PluginView::Window(Box::new(FtpSftpSshWindow { panel })))
    }

    fn close_idle(&mut self) {
        // Keep the runtime alive so session state survives plugin reopen.
    }
}

struct FtpSftpSshWindow {
    panel: Entity<view::FtpSftpSshView>,
}

impl WindowView for FtpSftpSshWindow {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "远程管理工作区".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.panel.clone().into_any_element()
    }
}

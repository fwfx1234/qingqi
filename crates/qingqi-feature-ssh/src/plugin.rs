//! 插件装配

use std::sync::Arc;

use anyhow::Result;
use gpui::{App, AppContext, Entity, IntoElement, Window};

use crate::manifest;
use crate::service::SshService;
use crate::view::SshView;
use qingqi_plugin::{
    command::Command,
    plugin::{Plugin, PluginCx, PluginId, PluginView, WindowView},
};

pub struct SshPlugin {
    pub service: Arc<SshService>,
}

impl Plugin for SshPlugin {
    fn manifest(&self) -> qingqi_plugin::plugin::Manifest {
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

    fn open(&mut self, cx: &mut PluginCx<'_>) -> Result<PluginView> {
        let view = cx.app.new(|cx| SshView::new(Arc::clone(&self.service), cx));

        Ok(PluginView::Window(Box::new(SshWindowView { view })))
    }

    fn start_background(&mut self, _cx: &mut PluginCx<'_>) {}

    fn shutdown(&mut self) {
        self.service.shutdown_all();
    }
}

struct SshWindowView {
    view: Entity<SshView>,
}

impl WindowView for SshWindowView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "远程管理".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> gpui::AnyElement {
        self.view.clone().into_any_element()
    }
}

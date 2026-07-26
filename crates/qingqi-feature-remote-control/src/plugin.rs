use std::sync::Arc;

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};
use qingqi_plugin::{
    command::Command,
    plugin::{Manifest, Plugin, PluginCx, PluginId, PluginView, WindowView},
};

use crate::manifest::{self, PLUGIN_ID};
use crate::service::RemoteControlService;

pub struct RemoteControlPlugin {
    service: Arc<RemoteControlService>,
}

impl RemoteControlPlugin {
    pub fn new(paths: qingqi_plugin::storage::AppPaths) -> anyhow::Result<Self> {
        Ok(Self {
            service: Arc::new(RemoteControlService::new(paths)),
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
            let mut view = super::view::RemoteControlView::new(Arc::clone(&self.service));
            view.init(cx);
            view
        });
        Ok(PluginView::Window(Box::new(RemoteControlWindow { view })))
    }

    fn start_background(&mut self, _cx: &mut PluginCx<'_>) {
        // Optionally auto-start the server
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

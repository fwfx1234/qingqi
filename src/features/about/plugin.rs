use gpui::{AnyElement, App, IntoElement, Window};

use crate::{
    app::events::AppEventBus,
    core::plugin::{PluginManifest, PluginRuntime, PluginSession},
    features::about::{manifest, view::AboutPage},
};

pub struct AboutRuntime;

impl AboutRuntime {
    pub fn new() -> Self {
        Self
    }
}

impl PluginRuntime for AboutRuntime {
    fn manifest(&self) -> PluginManifest {
        manifest::manifest()
    }

    fn open_session(
        &mut self,
        _: AppEventBus,
        _: &mut App,
    ) -> anyhow::Result<Box<dyn PluginSession>> {
        Ok(Box::new(AboutSession))
    }

    fn close_idle(&mut self) {}
}

struct AboutSession;

impl PluginSession for AboutSession {
    fn plugin_id(&self) -> &'static str {
        manifest::PLUGIN_ID
    }

    fn title(&self) -> &'static str {
        "关于"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        AboutPage.into_any_element()
    }
}

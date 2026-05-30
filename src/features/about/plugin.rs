use gpui::{App, IntoElement, Window};
use std::sync::Arc;

use crate::{
    core::plugin::{InlineView, Manifest, Plugin, PluginCx, PluginId, PluginView},
    features::about::{manifest, view::AboutPage},
};

pub struct AboutPlugin;

impl Plugin for AboutPlugin {
    fn manifest(&self) -> Manifest {
        manifest::manifest()
    }

    fn open(&mut self, _cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        Ok(PluginView::Inline(Box::new(AboutView)))
    }
}

struct AboutView;

impl InlineView for AboutView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "关于".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> gpui::AnyElement {
        AboutPage.into_any_element()
    }
}

use gpui::{App, AppContext, Entity, IntoElement, Window};
use std::sync::Arc;

use crate::{manifest, view};
use qingqi_plugin::plugin::{InlineView, Manifest, Plugin, PluginCx, PluginId, PluginView};

pub struct AboutPlugin;

impl Plugin for AboutPlugin {
    fn manifest(&self) -> Manifest {
        manifest::manifest()
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let panel = cx.app.new(|_cx| view::AboutView);
        Ok(PluginView::Inline(Box::new(AboutView { panel })))
    }
}

struct AboutView {
    panel: Entity<view::AboutView>,
}

impl InlineView for AboutView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "关于".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> gpui::AnyElement {
        self.panel.clone().into_any_element()
    }
}

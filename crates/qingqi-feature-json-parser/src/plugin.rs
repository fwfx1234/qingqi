use std::sync::Arc;

use gpui::{App, AppContext, Entity, IntoElement, Window};

use crate::{manifest, view};
use qingqi_plugin::{
    command::{ClipboardPayload, Command, ContextKind, ContextMatcher},
    plugin::{
        InlineView, Manifest, Plugin, PluginCx, PluginId, PluginView, recommended_plugin_command,
    },
};

pub struct JsonParserPlugin;

pub fn runtime() -> JsonParserPlugin {
    JsonParserPlugin
}

impl Plugin for JsonParserPlugin {
    fn manifest(&self) -> Manifest {
        manifest::manifest()
    }

    fn commands(&self, _query: &str) -> Vec<Command> {
        recommended_plugin_command(
            self.manifest(),
            [ContextMatcher::new(ContextKind::Json, 180)],
        )
    }

    fn clipboard_boost(&self, payload: &ClipboardPayload) -> Option<i32> {
        let text = payload.text.as_deref()?;
        let trimmed = text.trim();
        if (trimmed.starts_with('{') || trimmed.starts_with('['))
            && serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
        {
            Some(100)
        } else {
            None
        }
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let panel = cx.app.new(|_cx| view::JsonView::new());
        Ok(PluginView::Inline(Box::new(JsonParserView { panel })))
    }
}

struct JsonParserView {
    panel: Entity<view::JsonView>,
}

impl InlineView for JsonParserView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "JSON 解析".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> gpui::AnyElement {
        self.panel.clone().into_any_element()
    }

    fn on_input_changed(&mut self, text: &str, cx: &mut App) {
        self.panel
            .update(cx, |panel, cx| panel.set_launch_input(text, cx));
    }

    fn on_close(&mut self) {}
}

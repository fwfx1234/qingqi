use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};

use gpui::{App, IntoElement, Window};

use crate::{
    core::{
        command::{ClipboardPayload, Command, ContextKind, ContextMatcher},
        plugin::{
            InlineView, Manifest, Plugin, PluginCx, PluginId, PluginView,
            recommended_plugin_command,
        },
    },
    features::json_parser::{manifest, view},
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

    fn open(&mut self, _cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        Ok(PluginView::Inline(Box::new(JsonParserView {
            panel: Rc::new(RefCell::new(view::JsonView::new())),
        })))
    }
}

struct JsonParserView {
    panel: Rc<RefCell<view::JsonView>>,
}

impl InlineView for JsonParserView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "JSON 解析".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> gpui::AnyElement {
        view::JsonParserElement {
            panel: Rc::clone(&self.panel),
        }
        .into_any_element()
    }

    fn on_input_changed(&mut self, text: &str, cx: &mut App) {
        self.panel.borrow_mut().set_launch_input(text, cx);
    }

    fn on_close(&mut self) {
        self.panel.borrow_mut().clear();
    }
}

use std::{cell::RefCell, rc::Rc};

use gpui::{AnyElement, App, IntoElement, Window};

use crate::{
    app::events::AppEventBus,
    core::{
        command::{CommandItem, ContextKind, ContextMatcher},
        plugin::{PluginListItem, PluginManifest, PluginRuntime, PluginSession},
    },
    features::json_parser::{manifest, view},
};

pub struct JsonParserRuntime;

impl JsonParserRuntime {
    pub fn new() -> Self {
        Self
    }
}

impl PluginRuntime for JsonParserRuntime {
    fn manifest(&self) -> PluginManifest {
        manifest::manifest()
    }

    fn commands(&self) -> Vec<CommandItem> {
        let manifest = self.manifest();
        vec![
            CommandItem::plugin_open(
                manifest.id,
                manifest.name,
                manifest.description,
                manifest.keywords.iter().copied(),
                manifest.command_prefixes.iter().copied(),
                manifest.visual.icon,
            )
            .with_recommend_matchers([
                ContextMatcher::new(ContextKind::Json, 180),
                ContextMatcher::clipboard(ContextKind::Json, 100),
            ]),
        ]
    }

    fn open_session(
        &mut self,
        _: AppEventBus,
        _: &mut App,
    ) -> anyhow::Result<Box<dyn PluginSession>> {
        Ok(Box::new(JsonParserSession {
            panel: Rc::new(RefCell::new(view::JsonPanel::new())),
        }))
    }

    fn close_idle(&mut self) {}
}

struct JsonParserSession {
    panel: Rc<RefCell<view::JsonPanel>>,
}

impl PluginSession for JsonParserSession {
    fn plugin_id(&self) -> &'static str {
        manifest::PLUGIN_ID
    }

    fn title(&self) -> &'static str {
        "JSON 解析"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        view::JsonParserElement {
            panel: Rc::clone(&self.panel),
        }
        .into_any_element()
    }

    fn on_input_changed(&mut self, text: &str, cx: &mut App) -> Vec<PluginListItem> {
        self.panel.borrow_mut().set_launch_input(text, cx);
        Vec::new()
    }

    fn on_close(&mut self) {
        self.panel.borrow_mut().clear();
    }
}

use std::{cell::RefCell, rc::Rc};

use gpui::{AnyElement, App, IntoElement, Window};

use crate::{
    app::events::AppEventBus,
    core::{
        command::{CommandItem, ContextKind, ContextMatcher},
        plugin::{PluginListItem, PluginManifest, PluginRuntime, PluginSession},
        storage::AppPaths,
    },
    features::qr_code::{manifest, view},
};

pub struct QrCodeRuntime {
    paths: AppPaths,
}

impl QrCodeRuntime {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }
}

impl PluginRuntime for QrCodeRuntime {
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
                ContextMatcher::new(ContextKind::Url, 120),
                ContextMatcher::clipboard(ContextKind::Url, 80),
            ]),
        ]
    }

    fn open_session(
        &mut self,
        _: AppEventBus,
        _: &mut App,
    ) -> anyhow::Result<Box<dyn PluginSession>> {
        Ok(Box::new(QrCodeSession {
            panel: Rc::new(RefCell::new(view::QrPanel::new(self.paths.clone())?)),
        }))
    }

    fn close_idle(&mut self) {}
}

struct QrCodeSession {
    panel: Rc<RefCell<view::QrPanel>>,
}

impl PluginSession for QrCodeSession {
    fn plugin_id(&self) -> &'static str {
        manifest::PLUGIN_ID
    }

    fn title(&self) -> &'static str {
        "二维码"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        view::QrCodeElement {
            panel: Rc::clone(&self.panel),
        }
        .into_any_element()
    }

    fn on_input_changed(&mut self, text: &str, cx: &mut App) -> Vec<PluginListItem> {
        self.panel.borrow_mut().set_launch_input(text, cx);
        Vec::new()
    }

    fn on_close(&mut self) {
        self.panel.borrow_mut().clear_view_state();
    }
}

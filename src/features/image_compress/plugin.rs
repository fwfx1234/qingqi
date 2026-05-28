use std::{cell::RefCell, rc::Rc};

use gpui::{AnyElement, App, IntoElement, Window};

use crate::{
    app::events::AppEventBus,
    core::{
        command::{CommandItem, ContextKind, ContextMatcher},
        plugin::{PluginListItem, PluginManifest, PluginRuntime, PluginSession},
        storage::AppPaths,
    },
    features::image_compress::{manifest, service::ImageCompressService, view},
};

pub struct ImageCompressRuntime {
    paths: AppPaths,
}

impl ImageCompressRuntime {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }
}

impl PluginRuntime for ImageCompressRuntime {
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
                ContextMatcher::new(ContextKind::ImageFile, 180),
                ContextMatcher::clipboard(ContextKind::Image, 160),
                ContextMatcher::clipboard(ContextKind::ImageFile, 130),
            ]),
        ]
    }

    fn open_session(
        &mut self,
        _: AppEventBus,
        _: &mut App,
    ) -> anyhow::Result<Box<dyn PluginSession>> {
        Ok(Box::new(ImageCompressSession {
            panel: Rc::new(RefCell::new(view::ImageCompressPanel::new(
                ImageCompressService::new(self.paths.clone())?,
            ))),
        }))
    }

    fn close_idle(&mut self) {}
}

struct ImageCompressSession {
    panel: Rc<RefCell<view::ImageCompressPanel>>,
}

impl PluginSession for ImageCompressSession {
    fn plugin_id(&self) -> &'static str {
        manifest::PLUGIN_ID
    }

    fn title(&self) -> &'static str {
        "图片压缩"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        view::ImageCompressElement {
            panel: Rc::clone(&self.panel),
        }
        .into_any_element()
    }

    fn on_input_changed(&mut self, text: &str, _cx: &mut App) -> Vec<PluginListItem> {
        self.panel.borrow_mut().import_from_launch_input(text);
        Vec::new()
    }

    fn on_close(&mut self) {
        self.panel.borrow_mut().clear_transient_state();
    }
}

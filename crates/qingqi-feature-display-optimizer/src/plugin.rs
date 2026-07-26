use std::sync::Arc;

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};
use qingqi_plugin::plugin::{InlineView, Manifest, Plugin, PluginCx, PluginId, PluginView};

use crate::{manifest, service::DisplayOptimizerService, view::DisplayOptimizerView};

pub struct DisplayOptimizerPlugin {
    service: Arc<DisplayOptimizerService>,
}

impl DisplayOptimizerPlugin {
    pub fn new(service: Arc<DisplayOptimizerService>) -> Self {
        Self { service }
    }
}

impl Plugin for DisplayOptimizerPlugin {
    fn manifest(&self) -> Manifest {
        manifest::manifest()
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let service = Arc::clone(&self.service);
        let view = cx.app.new(|cx| DisplayOptimizerView::new(service, cx));
        Ok(PluginView::Inline(Box::new(DisplayOptimizerInlineView {
            view,
        })))
    }
}

struct DisplayOptimizerInlineView {
    view: Entity<DisplayOptimizerView>,
}

impl InlineView for DisplayOptimizerInlineView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "外接屏优化".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.view.clone().into_any_element()
    }
}

use std::sync::Arc;

use gpui::{App, AppContext, Entity, IntoElement, Window};

use crate::{manifest, view};
use qingqi_plugin::{
    command::{Command, ContextKind, ContextMatcher},
    plugin::{
        InlineView, Manifest, Plugin, PluginCx, PluginId, PluginView, recommended_plugin_command,
    },
    storage::AppPaths,
};

pub struct QrCodePlugin {
    paths: AppPaths,
}

pub fn runtime(paths: AppPaths) -> QrCodePlugin {
    QrCodePlugin { paths }
}

impl Plugin for QrCodePlugin {
    fn manifest(&self) -> Manifest {
        manifest::manifest()
    }

    fn commands(&self, _query: &str) -> Vec<Command> {
        recommended_plugin_command(
            self.manifest(),
            [ContextMatcher::new(ContextKind::Url, 120)],
        )
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let mut view = view::QrView::new(self.paths.clone())?;
        let panel = cx.app.new(|cx| {
            view.ensure_inputs(cx);
            view
        });
        Ok(PluginView::Inline(Box::new(QrCodeView { panel })))
    }
}

struct QrCodeView {
    panel: Entity<view::QrView>,
}

impl InlineView for QrCodeView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "二维码".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> gpui::AnyElement {
        self.panel.clone().into_any_element()
    }

    fn on_input_changed(&mut self, text: &str, cx: &mut App) {
        self.panel.update(cx, |this, cx| {
            this.set_launch_input(text, cx);
        });
    }

    fn on_close(&mut self) {
        // Entity is freshly created each time open() is called,
        // so explicit state clearing is not needed.
    }
}

use std::sync::Arc;
use std::path::Path;

use gpui::{App, AppContext, Entity, IntoElement, Window};

use crate::{manifest, service::ImageCompressService, view};
use qingqi_plugin::{
    command::{ClipboardPayload, Command, ContextKind, ContextMatcher},
    plugin::{
        InlineView, Manifest, Plugin, PluginCx, PluginId, PluginView, recommended_plugin_command,
    },
    storage::AppPaths,
};

pub struct ImageCompressPlugin {
    paths: AppPaths,
}

pub fn runtime(paths: AppPaths) -> ImageCompressPlugin {
    ImageCompressPlugin { paths }
}

impl Plugin for ImageCompressPlugin {
    fn manifest(&self) -> Manifest {
        manifest::manifest()
    }

    fn commands(&self, _query: &str) -> Vec<Command> {
        recommended_plugin_command(
            self.manifest(),
            [ContextMatcher::new(ContextKind::ImageFile, 180)],
        )
    }

    fn clipboard_boost(&self, payload: &ClipboardPayload) -> Option<i32> {
        if payload.image_path.is_some() {
            return Some(160);
        }
        if let Some(paths) = &payload.file_paths {
            if paths.iter().any(|p| looks_like_image_path(p)) {
                return Some(130);
            }
        }
        None
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let service = ImageCompressService::new(self.paths.clone())?;
        let panel = cx.app.new(|_cx| view::ImageCompressView::new(service));
        Ok(PluginView::Inline(Box::new(ImageCompressInlineView {
            panel,
        })))
    }
}

struct ImageCompressInlineView {
    panel: Entity<view::ImageCompressView>,
}

impl InlineView for ImageCompressInlineView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "图片压缩".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> gpui::AnyElement {
        self.panel.clone().into_any_element()
    }

    fn on_input_changed(&mut self, text: &str, cx: &mut App) {
        self.panel.update(cx, |panel, _cx| {
            panel.import_from_launch_input(text);
        });
    }

    fn on_close(&mut self) {
        // Entity state will be naturally dropped when the entity is no longer referenced.
    }
}

fn looks_like_image_path(path: &str) -> bool {
    let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" | "tif" | "avif" | "heic"
    )
}

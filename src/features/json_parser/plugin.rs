use std::{cell::RefCell, rc::Rc};

use gpui::{App, IntoElement, Window};

use crate::{
    core::{
        command::{CommandItem, ContextKind, ContextMatcher},
        plugin::{
            ConfiguredPluginRuntime, PanelPluginView, PluginCx, PluginManifest, PluginView,
            recommended_plugin_command,
        },
    },
    features::json_parser::{manifest, view},
};

pub type JsonParserRuntime = ConfiguredPluginRuntime<()>;

pub fn runtime() -> JsonParserRuntime {
    ConfiguredPluginRuntime::new(manifest::manifest)
        .with_commands(commands)
        .with_view(open_view)
}

fn commands(manifest: PluginManifest) -> Vec<CommandItem> {
    recommended_plugin_command(
        manifest,
        [
            ContextMatcher::new(ContextKind::Json, 180),
            ContextMatcher::clipboard(ContextKind::Json, 100),
        ],
    )
}

fn open_view(_: &mut (), _: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
    Ok(PluginView::Inline(Box::new(
        PanelPluginView::new(
            manifest::PLUGIN_ID,
            "JSON 解析",
            Rc::new(RefCell::new(view::JsonPanel::new())),
            |panel, _window: &mut Window, _cx: &mut App| {
                view::JsonParserElement {
                    panel: Rc::clone(panel),
                }
                .into_any_element()
            },
        )
        .with_input_changed(|panel, text, cx| {
            panel.borrow_mut().set_launch_input(text, cx);
        })
        .with_close(|panel| panel.borrow_mut().clear()),
    )))
}

use std::{cell::RefCell, rc::Rc};

use gpui::{App, IntoElement, Window};

use crate::{
    core::{
        command::{CommandItem, ContextKind, ContextMatcher},
        plugin::{
            ConfiguredPluginRuntime, PanelPluginView, PluginCx, PluginManifest, PluginView,
            recommended_plugin_command,
        },
        storage::AppPaths,
    },
    features::qr_code::{manifest, view},
};

pub type QrCodeRuntime = ConfiguredPluginRuntime<AppPaths>;

pub fn runtime(paths: AppPaths) -> QrCodeRuntime {
    ConfiguredPluginRuntime::with_state(manifest::manifest, paths)
        .with_commands(commands)
        .with_view(open_view)
}

fn commands(manifest: PluginManifest) -> Vec<CommandItem> {
    recommended_plugin_command(
        manifest,
        [
            ContextMatcher::new(ContextKind::Url, 120),
            ContextMatcher::clipboard(ContextKind::Url, 80),
        ],
    )
}

fn open_view(paths: &mut AppPaths, _: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
    Ok(PluginView::Inline(Box::new(
        PanelPluginView::new(
            manifest::PLUGIN_ID,
            "二维码",
            Rc::new(RefCell::new(view::QrPanel::new(paths.clone())?)),
            |panel, _window: &mut Window, _cx: &mut App| {
                view::QrCodeElement {
                    panel: Rc::clone(panel),
                }
                .into_any_element()
            },
        )
        .with_input_changed(|panel, text, cx| {
            panel.borrow_mut().set_launch_input(text, cx);
        })
        .with_close(|panel| panel.borrow_mut().clear_view_state()),
    )))
}

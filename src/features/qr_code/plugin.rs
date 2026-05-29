use std::{cell::RefCell, rc::Rc};

use gpui::{App, IntoElement, Window};

use crate::{
    app::events::AppEventBus,
    core::{
        command::{CommandItem, ContextKind, ContextMatcher},
        plugin::{
            ConfiguredPluginRuntime, PanelPluginSession, PluginListItem, PluginManifest,
            PluginSession, recommended_plugin_command,
        },
        storage::AppPaths,
    },
    features::qr_code::{manifest, view},
};

pub type QrCodeRuntime = ConfiguredPluginRuntime<AppPaths>;

pub fn runtime(paths: AppPaths) -> QrCodeRuntime {
    ConfiguredPluginRuntime::with_state(manifest::manifest, paths)
        .with_commands(commands)
        .with_session(open_session)
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

fn open_session(
    paths: &mut AppPaths,
    _: AppEventBus,
    _: &mut App,
) -> anyhow::Result<Box<dyn PluginSession>> {
    Ok(Box::new(
        PanelPluginSession::new(
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
            Vec::<PluginListItem>::new()
        })
        .with_close(|panel| panel.borrow_mut().clear_view_state()),
    ))
}

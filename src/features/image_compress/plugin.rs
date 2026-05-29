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
    features::image_compress::{manifest, service::ImageCompressService, view},
};

pub type ImageCompressRuntime = ConfiguredPluginRuntime<AppPaths>;

pub fn runtime(paths: AppPaths) -> ImageCompressRuntime {
    ConfiguredPluginRuntime::with_state(manifest::manifest, paths)
        .with_commands(commands)
        .with_session(open_session)
}

fn commands(manifest: PluginManifest) -> Vec<CommandItem> {
    recommended_plugin_command(
        manifest,
        [
            ContextMatcher::new(ContextKind::ImageFile, 180),
            ContextMatcher::clipboard(ContextKind::Image, 160),
            ContextMatcher::clipboard(ContextKind::ImageFile, 130),
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
            "图片压缩",
            Rc::new(RefCell::new(view::ImageCompressPanel::new(
                ImageCompressService::new(paths.clone())?,
            ))),
            |panel, _window: &mut Window, _cx: &mut App| {
                view::ImageCompressElement {
                    panel: Rc::clone(panel),
                }
                .into_any_element()
            },
        )
        .with_input_changed(|panel, text, _cx| {
            panel.borrow_mut().import_from_launch_input(text);
            Vec::<PluginListItem>::new()
        })
        .with_close(|panel| panel.borrow_mut().clear_transient_state()),
    ))
}

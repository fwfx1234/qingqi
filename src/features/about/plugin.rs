use gpui::{App, IntoElement, Window};

use crate::{
    app::events::AppEventBus,
    core::plugin::{
        ConfiguredPluginRuntime, PanelPluginSession, PluginSession,
    },
    features::about::{manifest, view::AboutPage},
};

pub type AboutRuntime = ConfiguredPluginRuntime<()>;

pub fn runtime() -> AboutRuntime {
    ConfiguredPluginRuntime::new(manifest::manifest).with_session(open_session)
}

fn open_session(
    _: &mut (),
    _: AppEventBus,
    _: &mut App,
) -> anyhow::Result<Box<dyn PluginSession>> {
    Ok(Box::new(PanelPluginSession::new(
        manifest::PLUGIN_ID,
        "关于",
        (),
        |_, _window: &mut Window, _cx: &mut App| AboutPage.into_any_element(),
    )))
}

use gpui::{App, IntoElement, Window};

use crate::{
    core::plugin::{ConfiguredPluginRuntime, PanelPluginView, PluginCx, PluginView},
    features::about::{manifest, view::AboutPage},
};

pub type AboutRuntime = ConfiguredPluginRuntime<()>;

pub fn runtime() -> AboutRuntime {
    ConfiguredPluginRuntime::new(manifest::manifest).with_view(open_view)
}

fn open_view(_: &mut (), _: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
    Ok(PluginView::Inline(Box::new(PanelPluginView::new(
        manifest::PLUGIN_ID,
        "关于",
        (),
        |_, _window: &mut Window, _cx: &mut App| AboutPage.into_any_element(),
    ))))
}

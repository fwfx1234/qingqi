use gpui::{IntoElement, ParentElement, Styled, div, px};

use qingqi_ui::{theme, ui};

pub fn workspace_chrome_config() -> ui::WindowChromeConfig {
    ui::WindowChromeConfig::new()
        .title("远程管理工作区")
        .titlebar_slot_alignment(ui::WindowChromeTitlebarSlotAlignment::Leading)
        .transparent(true)
}

pub fn titlebar_pill(text: impl Into<String>, emphasized: bool) -> impl IntoElement {
    let label = text.into();
    div()
        .rounded(px(6.0))
        .bg(if emphasized {
            theme::rgba_with_alpha(
                theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Cyan),
                0.16,
            )
        } else {
            theme::rgba_with_alpha(theme::semantic().bg_elevated, 0.42)
        })
        .border_1()
        .border_color(theme::rgba_with_alpha(
            theme::semantic().border_default,
            if emphasized { 0.36 } else { 0.22 },
        ))
        .px(px(8.0))
        .py(px(3.0))
        .text_size(px(10.0))
        .font_weight(if emphasized {
            gpui::FontWeight::SEMIBOLD
        } else {
            gpui::FontWeight::MEDIUM
        })
        .text_color(if emphasized {
            theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Cyan)
        } else {
            theme::semantic().text_secondary
        })
        .child(label)
}

use gpui::{App, ParentElement, Styled, div, px};
use gpui_component::theme::Theme;

use crate::{theme, ui};
use qingqi_plugin::plugin_spec::PluginAccent;

/// Unified chip component — replaces locally defined mode_chip / filter_chip / kind_chip / segmented_chip.
/// Returns `gpui::Div` so callers can chain `.id("...").on_click(...)` as needed.
pub fn chip(
    label: impl Into<gpui::SharedString>,
    selected: bool,
    accent: PluginAccent,
    cx: &App,
) -> gpui::Div {
    let label = label.into();
    let accent_color = ui::accent_color(accent);
    let dark = Theme::global(cx).is_dark();
    let accent_soft = if dark {
        theme::accent_soft_dark(accent)
    } else {
        theme::accent_soft(accent)
    };

    div()
        .px_2()
        .h(px(26.0))
        .rounded(theme::radius_sm())
        .bg(if selected {
            accent_soft
        } else {
            ui::bg_subtle(cx)
        })
        .border_1()
        .border_color(if selected {
            let accent_hsla: gpui::Hsla = accent_color.into();
            accent_hsla
        } else {
            ui::border_light(cx)
        })
        .text_color(if selected {
            accent_color
        } else {
            ui::text_secondary(cx)
        })
        .text_size(theme::font_size_caption())
        .font_weight(gpui::FontWeight::MEDIUM)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(label)
}

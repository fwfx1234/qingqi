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

    let bg: gpui::Hsla = if selected {
        accent_soft.into()
    } else {
        ui::bg_subtle(cx)
    };
    let border: gpui::Hsla = if selected {
        accent_color.into()
    } else {
        ui::border_light(cx)
    };
    let text: gpui::Hsla = if selected {
        accent_color.into()
    } else {
        ui::text_secondary(cx)
    };

    div()
        .px_2()
        .h(px(26.0))
        .rounded(theme::radius_sm())
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_color(text)
        .text_size(theme::font_size_caption())
        .font_weight(gpui::FontWeight::MEDIUM)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(label)
}

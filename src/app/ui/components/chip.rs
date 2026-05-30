use gpui::{IntoElement, ParentElement, Styled, div, px};

use crate::app::{theme, ui};
use crate::core::plugin_spec::PluginAccent;

/// Unified chip component — replaces locally defined mode_chip / filter_chip / kind_chip / segmented_chip.
/// Returns `gpui::Div` so callers can chain `.id("...").on_click(...)` as needed.
pub fn chip(
    label: impl Into<gpui::SharedString>,
    selected: bool,
    accent: PluginAccent,
    dark: bool,
) -> gpui::Div {
    let label = label.into();
    let accent_color = ui::accent_color(accent);
    let accent_soft = if dark {
        let theme_accent = ui::accent_to_theme(accent);
        theme::accent_soft_dark(theme_accent)
    } else {
        let theme_accent = ui::accent_to_theme(accent);
        theme::accent_soft(theme_accent)
    };

    div()
        .px_2()
        .h(px(26.0))
        .rounded(theme::radius_sm())
        .bg(if selected {
            accent_soft
        } else {
            ui::bg_subtle()
        })
        .border_1()
        .border_color(if selected {
            let accent_hsla: gpui::Hsla = accent_color.into();
            accent_hsla
        } else {
            ui::border_light()
        })
        .text_color(if selected {
            accent_color
        } else {
            ui::text_secondary()
        })
        .text_size(theme::font_size_caption())
        .font_weight(gpui::FontWeight::MEDIUM)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(label)
}

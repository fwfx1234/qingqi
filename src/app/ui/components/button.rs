use gpui::{IntoElement, ParentElement, SharedString, Styled, div, hsla, px};

use crate::app::{theme, ui};
use crate::core::plugin_spec::PluginAccent;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

/// Unified button — replaces locally defined primary_button / action_button / ghost_button.
/// Returns `gpui::Div` so callers can chain `.id("...").hover(...).on_click(...)` as needed.
pub fn button(
    label: impl Into<SharedString>,
    variant: ButtonVariant,
    accent: Option<PluginAccent>,
    _dark: bool,
) -> gpui::Div {
    let label = label.into();
    let accent_color = accent.unwrap_or(PluginAccent::Blue);
    let (bg, text, border): (gpui::Hsla, gpui::Hsla, gpui::Hsla) = match variant {
        ButtonVariant::Primary => {
            let accent: gpui::Hsla = ui::accent_color(accent_color).into();
            let white: gpui::Hsla = ui::white().into();
            (accent, white, accent)
        }
        ButtonVariant::Secondary => (
            ui::bg_surface().into(),
            ui::text_primary().into(),
            ui::border_light(),
        ),
        ButtonVariant::Ghost => {
            let transparent = hsla(0.0, 0.0, 0.0, 0.0);
            (transparent, ui::text_primary().into(), transparent)
        }
        ButtonVariant::Danger => {
            let danger: gpui::Hsla = ui::danger().into();
            let white: gpui::Hsla = ui::white().into();
            (danger, white, danger)
        }
    };

    let mut btn = div()
        .h(px(32.0))
        .px(px(12.0))
        .rounded(theme::radius_md())
        .flex()
        .items_center()
        .justify_center()
        .gap_1()
        .text_size(theme::font_size_body())
        .text_color(text)
        .child(label);

    if variant != ButtonVariant::Ghost {
        btn = btn.bg(bg).border_1().border_color(border);
    }

    btn
}

/// Unified icon button — returns a styled element for icons.
pub fn icon_button(icon_svg: &str, dark: bool, size_px: f32) -> gpui::Div {
    let color = if dark {
        theme::slate_400()
    } else {
        theme::slate_500()
    };
    let icon_size = size_px * 0.55;

    div()
        .size(px(size_px))
        .rounded(theme::radius_md())
        .flex()
        .items_center()
        .justify_center()
        .child(ui::icon_element(icon_svg, color, icon_size))
}

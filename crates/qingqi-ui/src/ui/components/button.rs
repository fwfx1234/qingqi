use gpui::{App, ParentElement, SharedString, Styled, div, hsla, px};
use gpui_component::theme::Theme;

use crate::{theme, ui};
use qingqi_plugin::plugin_spec::PluginAccent;

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
    cx: &App,
) -> gpui::Div {
    let label = label.into();
    let t = Theme::global(cx);
    let (bg, text, border): (gpui::Hsla, gpui::Hsla, gpui::Hsla) = match variant {
        ButtonVariant::Primary => {
            let primary = if let Some(accent) = accent {
                ui::accent_color(accent).into()
            } else {
                t.primary
            };
            let white: gpui::Hsla = hsla(0., 0., 1., 1.);
            (primary, white, primary)
        }
        ButtonVariant::Secondary => (t.list, t.foreground, t.border),
        ButtonVariant::Ghost => {
            let transparent = hsla(0.0, 0.0, 0.0, 0.0);
            (transparent, ui::text_primary(cx), transparent)
        }
        ButtonVariant::Danger => {
            let danger: gpui::Hsla = ui::danger(cx);
            let white: gpui::Hsla = hsla(0., 0., 1., 1.);
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
pub fn icon_button(icon_svg: &str, size_px: f32, cx: &App) -> gpui::Div {
    let color: gpui::Hsla = Theme::global(cx).muted_foreground;
    let icon_size = size_px * 0.55;

    div()
        .size(px(size_px))
        .rounded(theme::radius_md())
        .flex()
        .items_center()
        .justify_center()
        .child(ui::icon_element(icon_svg, color.into(), icon_size))
}

use gpui::{IntoElement, ParentElement, Styled, div, px};

use crate::{theme, ui};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StatusTone {
    Neutral,
    Success,
    Warning,
    Danger,
    Info,
}

/// Unified status pill — replaces locally defined status_tag / status_chip / status_pill.
pub fn status_pill(label: impl Into<gpui::SharedString>, tone: StatusTone) -> impl IntoElement {
    let label = label.into();
    let color = match tone {
        StatusTone::Neutral => ui::text_secondary(),
        StatusTone::Success => ui::success(),
        StatusTone::Warning => ui::warning(),
        StatusTone::Danger => ui::danger(),
        StatusTone::Info => ui::info(),
    };

    div()
        .px_2()
        .h(px(24.0))
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(color, 0.14))
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::font_size_caption())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(color)
        .child(label)
}

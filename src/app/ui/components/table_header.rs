use gpui::{IntoElement, ParentElement, SharedString, Styled, div, px};

use crate::app::{theme, ui};

/// Shared fixed-width table header cell.
pub fn table_header_cell(label: impl Into<SharedString>, width: f32) -> impl IntoElement {
    let label = label.into();
    div()
        .w(px(width))
        .h(px(30.0))
        .px_2()
        .flex()
        .items_center()
        .text_size(theme::font_size_caption())
        .text_color(ui::text_secondary())
        .child(label)
}

/// Shared flexible-width table header cell.
/// `grow >= 2.0` uses flex-1, otherwise a fixed 96px width (matching local conventions).
pub fn table_header_flex(label: impl Into<SharedString>, grow: f32) -> impl IntoElement {
    let label = label.into();
    let cell = div()
        .h(px(30.0))
        .px_2()
        .flex()
        .items_center()
        .text_size(theme::font_size_caption())
        .text_color(ui::text_secondary())
        .child(label);
    if grow >= 2.0 {
        cell.flex_1().into_any_element()
    } else {
        cell.w(px(96.0)).into_any_element()
    }
}

use gpui::{IntoElement, ParentElement, Styled, div, px};

use crate::{theme, ui};

/// Unified empty state — icon SVG + title + description.
pub fn empty_state(
    icon_svg: &str,
    title: impl Into<gpui::SharedString>,
    description: impl Into<gpui::SharedString>,
    _dark: bool,
) -> impl IntoElement {
    let title = title.into();
    let desc = description.into();

    div()
        .w_full()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .flex_col()
        .gap(px(12.0))
        .child(ui::icon_element(icon_svg, ui::text_tertiary(), 48.0))
        .child(
            div()
                .text_size(theme::font_size_heading())
                .text_color(ui::text_primary())
                .child(title),
        )
        .child(
            div()
                .text_size(theme::font_size_body())
                .text_color(ui::text_secondary())
                .child(desc),
        )
}

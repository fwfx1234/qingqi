use gpui::{App, IntoElement, ParentElement, Styled, div, px};

use crate::{theme, ui};

/// Unified empty state — icon SVG + title + description.
pub fn empty_state(
    icon_svg: &str,
    title: impl Into<gpui::SharedString>,
    description: impl Into<gpui::SharedString>,
    cx: &App,
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
        .child(ui::icon_element(icon_svg, ui::text_tertiary(cx).into(), 48.0))
        .child(
            div()
                .text_size(theme::font_size_heading())
                .text_color(ui::text_primary(cx))
                .child(title),
        )
        .child(
            div()
                .text_size(theme::font_size_body())
                .text_color(ui::text_secondary(cx))
                .child(desc),
        )
}

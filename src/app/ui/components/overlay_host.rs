use gpui::{
    InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div,
};

use crate::app::ui;

/// Unified overlay host — replaces 4 independently implemented overlay_shell() functions.
pub fn overlay_host(
    _dark: bool,
    backdrop_id: &'static str,
    on_close: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    content: impl IntoElement,
) -> impl IntoElement {
    div()
        .size_full()
        .absolute()
        .top_0()
        .left_0()
        .child(
            div()
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .bg(ui::overlay_backdrop())
                .id(backdrop_id)
                .on_click(move |event, window, cx| on_close(event, window, cx)),
        )
        .child(
            div()
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .flex()
                .items_center()
                .justify_center()
                .child(content),
        )
}

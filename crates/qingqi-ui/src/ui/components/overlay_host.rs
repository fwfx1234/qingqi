use gpui::{
    InteractiveElement, IntoElement, ParentElement, Styled, div,
};

use crate::ui;

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
        .occlude()
        .child(
            div()
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .occlude()
                .bg(ui::overlay_backdrop())
                .id(backdrop_id)
                .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    on_close(&gpui::ClickEvent::default(), window, cx);
                }),
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
                .occlude()
                .child(
                    div()
                        .id("overlay-content")
                        .occlude()
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .child(content),
                ),
        )
}

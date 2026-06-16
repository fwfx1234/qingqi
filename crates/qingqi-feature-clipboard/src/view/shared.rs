use qingqi_ui::ui::components::toggle;

use super::*;

pub(super) fn theme_button(
    label: &'static str,
    _dark: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(label)
        .h(px(26.0))
        .px(px(8.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(theme::rgba_with_alpha(theme::semantic().bg_surface, 0.8))
        .hover(|style| style.bg(ui::row_hover()).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .text_color(theme::semantic().text_primary)
        .child(label)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

pub(super) fn pill_button(
    label: &'static str,
    handler: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(label)
        .h(px(26.0))
        .px(px(8.0))
        .rounded(px(13.0))
        .bg(theme::semantic().bg_subtle)
        .hover(|style| style.bg(ui::row_hover()).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .text_size(theme::font_size_caption())
        .text_color(theme::semantic().text_primary)
        .child(label)
        .on_click(move |event, _window, cx| handler(event, cx))
}

pub(super) fn toggle_control(
    enabled: bool,
    handler: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    toggle(enabled).id("toggle").on_click(move |event, _window, cx| handler(event, cx))
}

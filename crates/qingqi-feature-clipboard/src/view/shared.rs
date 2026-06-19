use gpui_component::switch::Switch;
use gpui_component::theme::Theme;

use super::*;

pub(super) fn theme_button(
    label: &'static str,
    cx: &App,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    let t = Theme::global(cx);
    div()
        .id(label)
        .h(px(26.0))
        .px(px(8.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(ui::border_light(cx))
        .bg(t.list)
        .hover(|style| style.bg(ui::row_hover(cx)).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .text_color(t.foreground)
        .child(label)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

pub(super) fn pill_button(
    label: &'static str,
    cx: &App,
    handler: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    let t = Theme::global(cx);
    div()
        .id(label)
        .h(px(26.0))
        .px(px(8.0))
        .rounded(px(13.0))
        .bg(t.muted)
        .hover(|style| style.bg(ui::row_hover(cx)).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .text_size(theme::font_size_caption())
        .text_color(t.foreground)
        .child(label)
        .on_click(move |event, _window, cx| handler(event, cx))
}

pub(super) fn toggle_control(
    id: impl Into<gpui::ElementId>,
    enabled: bool,
    handler: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    Switch::new(id)
        .checked(enabled)
        .on_click(move |_checked, _window, cx| handler(cx))
}

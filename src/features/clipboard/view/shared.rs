use super::*;

pub(super) fn header_action_button(
    id: &'static str,
    dark: bool,
    child: impl IntoElement,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .h(px(32.0))
        .rounded(px(10.0))
        .border_1()
        .border_color(theme::token("color-border-default", dark))
        .bg(theme::rgba_with_alpha(theme::launcher_accent(dark), 0.08))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .id(id)
        .min_w(px(32.0))
        .px(px(12.0))
        .text_size(px(12.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme::token("color-text-primary", dark))
        .hover(|style| {
            style
                .bg(theme::token("color-row-hover", dark))
                .cursor_pointer()
        })
        .child(child)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

pub(super) fn theme_button(
    label: &'static str,
    dark: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(label)
        .h(px(28.0))
        .px_3()
        .rounded(px(6.0))
        .border_1()
        .border_color(theme::token("color-border-default", dark))
        .bg(theme::token("color-bg-surface", dark))
        .hover(|style| {
            style
                .bg(theme::launcher_row_selected(dark))
                .cursor_pointer()
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(theme::token("color-text-primary", dark))
        .child(label)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

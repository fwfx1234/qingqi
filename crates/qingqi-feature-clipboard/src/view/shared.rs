use super::*;

pub(super) fn header_action_button(
    id: &'static str,
    _dark: bool,
    child: impl IntoElement,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .h(px(26.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(theme::rgba_with_alpha(
            ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue),
            0.08,
        ))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(4.0))
        .id(id)
        .min_w(px(26.0))
        .px(px(8.0))
        .text_size(px(10.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme::semantic().text_primary)
        .hover(|style| style.bg(theme::semantic().bg_hover).cursor_pointer())
        .child(child)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

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

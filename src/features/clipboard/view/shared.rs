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
        .border_color(theme::semantic(dark).border_default)
        .bg(theme::rgba_with_alpha(
            ui::accent_color(crate::core::plugin_spec::PluginAccent::Blue),
            0.08,
        ))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .id(id)
        .min_w(px(32.0))
        .px(px(12.0))
        .text_size(px(12.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme::semantic(dark).text_primary)
        .hover(|style| style.bg(theme::semantic(dark).row_hover).cursor_pointer())
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
        .border_color(theme::semantic(dark).border_default)
        .bg(theme::semantic(dark).bg_surface)
        .hover(|style| style.bg(ui::row_hover(dark)).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(theme::semantic(dark).text_primary)
        .child(label)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

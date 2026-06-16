use gpui::{px, rgb, IntoElement, ParentElement, Styled, div};
use qingqi_ui::{theme, ui};

pub fn transparent_surface() -> gpui::Hsla {
    theme::rgba_with_alpha(theme::semantic().bg_surface, 0.0)
}

pub fn api_accent() -> gpui::Rgba {
    theme::semantic().primary
}

pub fn content_split(_stacked: bool) -> gpui::Div {
    div().flex_1().min_h(px(0.0)).flex().flex_col()
}

pub fn section_micro_label(label: impl Into<String>, _dark: bool) -> impl IntoElement {
    div()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(ui::text_tertiary())
        .child(label.into())
}

pub fn response_metric(text: String, _dark: bool) -> impl IntoElement {
    div()
        .h(px(22.0))
        .px(px(8.0))
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(
            theme::semantic().text_secondary,
            0.08,
        ))
        .flex()
        .items_center()
        .text_size(px(10.0))
        .font_family("SF Mono")
        .text_color(theme::semantic().text_secondary)
        .child(text)
}

pub fn circle_badge(label: &str, color: u32, size: f32) -> impl IntoElement {
    div()
        .size(px(size))
        .rounded(px(size / 2.0))
        .bg(rgb(color))
        .text_color(theme::white())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px((size * 0.36).max(10.0)))
        .font_weight(gpui::FontWeight::BOLD)
        .child(label.to_string())
}

pub fn status_badge(
    response: &crate::service::ApiResponse,
    _dark: bool,
) -> impl IntoElement {
    let color = if response.status_code == 0 {
        theme::semantic().text_secondary
    } else if response.status_code >= 200 && response.status_code < 300 {
        theme::semantic().success
    } else {
        theme::semantic().danger
    };
    div()
        .px(px(10.0))
        .py(px(4.0))
        .rounded(px(6.0))
        .bg(theme::rgba_with_alpha(color, 0.10))
        .text_size(px(12.0))
        .font_family("SF Mono")
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(color)
        .child(response.status_line.clone())
}

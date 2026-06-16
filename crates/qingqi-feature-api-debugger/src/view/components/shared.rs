use gpui::{px, rgb, App, IntoElement, ParentElement, Styled, div};
use gpui_component::theme::Theme;
use qingqi_ui::{theme, ui};

pub fn transparent_surface(cx: &App) -> gpui::Hsla {
    theme::rgba_with_alpha(Theme::global(cx).list.into(), 0.0)
}

pub fn api_accent(cx: &App) -> gpui::Rgba {
    Theme::global(cx).primary.into()
}

pub fn content_split(_stacked: bool) -> gpui::Div {
    div().flex_1().min_h(px(0.0)).flex().flex_col()
}

pub fn section_micro_label(label: impl Into<String>, cx: &App) -> impl IntoElement {
    div()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(ui::text_tertiary(cx))
        .child(label.into())
}

pub fn response_metric(text: String, cx: &App) -> impl IntoElement {
    div()
        .h(px(22.0))
        .px(px(8.0))
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(
            Theme::global(cx).muted_foreground.into(),
            0.08,
        ))
        .flex()
        .items_center()
        .text_size(px(10.0))
        .font_family("SF Mono")
        .text_color(Theme::global(cx).muted_foreground)
        .child(text)
}

pub fn circle_badge(label: &str, color: u32, size: f32) -> impl IntoElement {
    div()
        .size(px(size))
        .rounded(px(size / 2.0))
        .bg(rgb(color))
        .text_color(gpui::hsla(0., 0., 1., 1.))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px((size * 0.36).max(10.0)))
        .font_weight(gpui::FontWeight::BOLD)
        .child(label.to_string())
}

pub fn status_badge(
    response: &crate::service::ApiResponse,
    cx: &App,
) -> impl IntoElement {
    let color = if response.status_code == 0 {
        Theme::global(cx).muted_foreground
    } else if response.status_code >= 200 && response.status_code < 300 {
        Theme::global(cx).success
    } else {
        Theme::global(cx).danger
    };
    div()
        .px(px(10.0))
        .py(px(4.0))
        .rounded(px(6.0))
        .bg(theme::rgba_with_alpha(color.into(), 0.10))
        .text_size(px(12.0))
        .font_family("SF Mono")
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(color)
        .child(response.status_line.clone())
}

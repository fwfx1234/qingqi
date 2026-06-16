use gpui::{App, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_component::theme::Theme;

use crate::theme;

/// Unified settings card — header with title + optional subtitle, content below.
/// Extracted from system_settings/view.rs for reuse across clipboard settings, download settings, etc.
pub fn settings_card(
    title: impl Into<SharedString>,
    subtitle: Option<impl Into<SharedString>>,
    content: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    let title = title.into();
    let t = Theme::global(cx);

    div()
        .rounded(theme::radius_lg())
        .border_1()
        .border_color(t.border)
        .bg(t.list)
        .flex()
        .flex_col()
        .child(
            // Header
            div()
                .px(theme::space_4())
                .py(theme::space_3())
                .border_b_1()
                .border_color(t.border)
                .bg(t.muted)
                .flex()
                .items_center()
                .justify_between()
                .child({
                    let mut header = div().flex().flex_col().gap_0p5().child(
                        div()
                            .text_size(theme::font_size_body())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(t.foreground)
                            .child(title.clone()),
                    );
                    if let Some(st) = subtitle {
                        header = header.child(
                            div()
                                .text_size(theme::font_size_caption())
                                .text_color(t.muted_foreground)
                                .child(st.into()),
                        );
                    }
                    header
                }),
        )
        .child(div().flex().flex_col().child(content))
}

/// Unified settings row — label + description on the left, control on the right.
pub fn settings_row(
    label: impl Into<SharedString>,
    description: impl Into<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    let label = label.into();
    let desc = description.into();
    let t = Theme::global(cx);

    div()
        .min_h(px(52.0))
        .px(theme::space_4())
        .py(theme::space_2())
        .border_b_1()
        .border_color(t.border)
        .flex()
        .items_center()
        .justify_between()
        .gap(theme::space_4())
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_size(theme::font_size_body())
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(t.foreground)
                        .child(label),
                )
                .child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(t.muted_foreground)
                        .child(desc),
                ),
        )
        .child(control)
}

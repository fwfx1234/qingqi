use crate::components::theme::ActiveTheme;
use crate::theme;
use gpui::{App, IntoElement, ParentElement, SharedString, Styled, div, px};

/// Unified settings card — header with title + optional subtitle, content below.
pub fn settings_card(
    title: impl Into<SharedString>,
    subtitle: Option<impl Into<SharedString>>,
    content: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    let title = title.into();
    let t = cx.theme();

    let mut header = div().flex().flex_col().gap_0p5().child(
        div()
            .text_size(theme::font_size_body())
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(t.foreground())
            .child(title),
    );
    if let Some(st) = subtitle {
        header = header.child(
            div()
                .text_size(theme::font_size_caption())
                .text_color(t.muted_foreground)
                .child(st.into()),
        );
    }

    div()
        .rounded(theme::radius_md())
        .px_3()
        .py_2()
        .border_1()
        .border_color(t.border())
        .flex()
        .flex_col()
        .gap_2()
        .child(header)
        .child(content)
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
    let t = cx.theme();

    div()
        .min_h(px(52.0))
        .px(theme::space_4())
        .py(theme::space_2())
        .border_b_1()
        .border_color(t.border())
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
                        .text_color(t.foreground())
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

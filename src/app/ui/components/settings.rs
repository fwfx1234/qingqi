use gpui::{IntoElement, ParentElement, SharedString, Styled, div, px};

use crate::app::theme;

/// Unified settings card — header with title + optional subtitle, content below.
/// Extracted from system_settings/view.rs for reuse across clipboard settings, download settings, etc.
pub fn settings_card(
    dark: bool,
    title: impl Into<SharedString>,
    subtitle: Option<impl Into<SharedString>>,
    content: impl IntoElement,
) -> impl IntoElement {
    let title = title.into();
    let s = theme::semantic(dark);

    div()
        .rounded(theme::radius_lg())
        .border_1()
        .border_color(s.border_default)
        .bg(s.bg_surface)
        .flex()
        .flex_col()
        .child(
            // Header
            div()
                .px(theme::space_4())
                .py(theme::space_3())
                .border_b_1()
                .border_color(s.border_default)
                .bg(s.bg_subtle_2)
                .flex()
                .items_center()
                .justify_between()
                .child({
                    let mut header = div().flex().flex_col().gap_0p5().child(
                        div()
                            .text_size(theme::font_size_body())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(s.text_primary)
                            .child(title.clone()),
                    );
                    if let Some(st) = subtitle {
                        header = header.child(
                            div()
                                .text_size(theme::font_size_caption())
                                .text_color(s.text_secondary)
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
    dark: bool,
    label: impl Into<SharedString>,
    description: impl Into<SharedString>,
    control: impl IntoElement,
) -> impl IntoElement {
    let label = label.into();
    let desc = description.into();
    let s = theme::semantic(dark);

    div()
        .min_h(px(52.0))
        .px(theme::space_4())
        .py(theme::space_2())
        .border_b_1()
        .border_color(s.border_default)
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
                        .text_color(s.text_primary)
                        .child(label),
                )
                .child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(s.text_secondary)
                        .child(desc),
                ),
        )
        .child(control)
}

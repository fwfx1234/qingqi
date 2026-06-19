use super::shared::{response_metric, status_badge, transparent_surface};
use crate::code_gen::CodeLanguage;
use crate::service::{ApiResponse, HttpHistory, ResponseTab};
use crate::view::ApiDebuggerView;
use gpui::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px,
};
use gpui_component::theme::Theme;
use gpui_component::{
    IconName, Sizable, Size,
    button::{Button, ButtonVariants},
};
use qingqi_ui::{theme, ui, ui::glass};

pub fn response_panel(
    view: Entity<ApiDebuggerView>,
    response_tab: ResponseTab,
    response: ApiResponse,
    response_text: String,
    history_entries: Vec<HttpHistory>,
    code_lang: CodeLanguage,
    notice: String,
    cx: &App,
) -> impl IntoElement {
    let tabs_view = view.clone();

    let content: AnyElement = match response_tab {
        ResponseTab::History => {
            response_history_view(view.clone(), history_entries, cx).into_any_element()
        }
        ResponseTab::Code => {
            response_code_view(view.clone(), code_lang, response_text, cx).into_any_element()
        }
        ResponseTab::Body => response_body_view(
            view.clone(),
            response.content_type.clone(),
            response_text,
            cx,
        )
        .into_any_element(),
        _ => response_text_view(response_text, cx).into_any_element(),
    };

    div()
        .h(px(310.0))
        .min_h(px(220.0))
        .max_h(px(380.0))
        .flex_none()
        .border_t_1()
        .border_color(glass::divider(cx))
        .bg(glass::panel(cx))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(12.0))
                .py(px(8.0))
                .border_b_1()
                .border_color(glass::divider(cx))
                .bg(glass::bar(cx))
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(status_badge(&response, cx))
                .child(div().flex_1())
                .child(response_metric(format!("{} ms", response.duration_ms), cx))
                .child(response_metric(format!("{} B", response.size_bytes), cx)),
        )
        .child(
            div()
                .px(px(10.0))
                .py(px(4.0))
                .border_b_1()
                .border_color(glass::divider(cx))
                .bg(glass::bar(cx))
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(4.0))
                .children(
                    ResponseTab::all()
                        .into_iter()
                        .enumerate()
                        .map(move |(index, tab)| {
                            let active = tab == response_tab;
                            let tab_view = tabs_view.clone();
                            div()
                                .id(("api-response-tab", index))
                                .px(px(9.0))
                                .py(px(5.0))
                                .rounded(px(4.0))
                                .bg(if active {
                                    theme::rgba_with_alpha(
                                        Theme::global(cx).foreground.into(),
                                        0.055,
                                    )
                                } else {
                                    transparent_surface(cx)
                                })
                                .text_size(px(11.0))
                                .text_color(if active {
                                    Theme::global(cx).foreground
                                } else {
                                    ui::text_tertiary(cx)
                                })
                                .hover(move |style| style.bg(glass::hover_bg(cx)).cursor_pointer())
                                .child(tab.label())
                                .on_click(move |_, window, cx| {
                                    tab_view.update(cx, |view, _cx| view.set_response_tab(tab));
                                    window.refresh();
                                })
                        }),
                ),
        )
        .child(content)
        .child(
            div()
                .px(px(12.0))
                .py(px(6.0))
                .border_t_1()
                .border_color(glass::divider(cx))
                .text_size(px(11.0))
                .text_color(ui::text_secondary(cx))
                .child(notice),
        )
}

fn response_text_view(text: String, cx: &App) -> impl IntoElement {
    div()
        .id("api-response-scroll")
        .flex_1()
        .min_h(px(0.0))
        .overflow_y_scroll()
        .scrollbar_width(px(4.0))
        .p(px(10.0))
        .bg(glass::inset(cx))
        .child(
            div()
                .font_family("SF Mono")
                .text_size(px(12.0))
                .line_height(px(18.0))
                .text_color(Theme::global(cx).muted_foreground)
                .child(text),
        )
}

fn response_body_view(
    view: Entity<ApiDebuggerView>,
    content_type: String,
    text: String,
    cx: &App,
) -> impl IntoElement {
    let binary = is_binary_content_type(&content_type);
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.0))
                .py(px(6.0))
                .border_b_1()
                .border_color(glass::divider(cx))
                .bg(glass::bar(cx))
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(response_action_button(
                    view.clone(),
                    "复制",
                    ResponseBodyAction::Copy,
                    cx,
                ))
                .child(response_action_button(
                    view.clone(),
                    "格式化",
                    ResponseBodyAction::Format,
                    cx,
                ))
                .child(response_action_button(
                    view.clone(),
                    "保存",
                    ResponseBodyAction::Save,
                    cx,
                ))
                .child(div().flex_1())
                .when(!content_type.is_empty(), |row| {
                    row.child(
                        div()
                            .text_size(px(10.0))
                            .font_family("SF Mono")
                            .text_color(ui::text_tertiary(cx))
                            .child(content_type.clone()),
                    )
                }),
        )
        .when(binary, |panel| {
            panel.child(
                div()
                    .px(px(10.0))
                    .py(px(6.0))
                    .bg(theme::rgba_with_alpha(
                        Theme::global(cx).danger.into(),
                        0.08,
                    ))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(px(11.0))
                    .text_color(Theme::global(cx).danger)
                    .child(
                        Button::new("api-response-binary-warning-icon")
                            .ghost()
                            .icon(IconName::TriangleAlert)
                            .with_size(Size::XSmall),
                    )
                    .child("二进制/图片响应，文本预览可能乱码，建议点击「保存」后查看"),
            )
        })
        .child(response_text_view(text, cx))
}

fn response_code_view(
    view: Entity<ApiDebuggerView>,
    code_lang: CodeLanguage,
    code_text: String,
    cx: &App,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.0))
                .py(px(6.0))
                .border_b_1()
                .border_color(glass::divider(cx))
                .bg(glass::bar(cx))
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(4.0))
                .children(
                    CodeLanguage::all()
                        .into_iter()
                        .enumerate()
                        .map(move |(index, lang)| {
                            let active = lang == code_lang;
                            let lang_view = view.clone();
                            div()
                                .id(("api-code-lang", index))
                                .px(px(8.0))
                                .py(px(3.0))
                                .rounded(px(4.0))
                                .text_size(px(11.0))
                                .bg(if active {
                                    theme::rgba_with_alpha(
                                        Theme::global(cx).foreground.into(),
                                        0.055,
                                    )
                                } else {
                                    transparent_surface(cx)
                                })
                                .text_color(if active {
                                    Theme::global(cx).foreground
                                } else {
                                    ui::text_tertiary(cx)
                                })
                                .hover(move |style| style.bg(glass::hover_bg(cx)).cursor_pointer())
                                .child(lang.label())
                                .on_click(move |_, window, cx| {
                                    lang_view
                                        .update(cx, |view, _cx| view.set_response_code_lang(lang));
                                    window.refresh();
                                })
                        }),
                ),
        )
        .child(response_text_view(code_text, cx))
}

fn response_history_view(
    view: Entity<ApiDebuggerView>,
    entries: Vec<HttpHistory>,
    cx: &App,
) -> impl IntoElement {
    let clear_view = view.clone();
    let count = entries.len();
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.0))
                .py(px(6.0))
                .border_b_1()
                .border_color(glass::divider(cx))
                .bg(glass::bar(cx))
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.0))
                        .text_color(ui::text_secondary(cx))
                        .child(format!("共 {count} 条历史记录")),
                )
                .when(count > 0, |row| {
                    row.child(
                        div()
                            .id("api-history-clear")
                            .px(px(8.0))
                            .py(px(3.0))
                            .rounded(px(4.0))
                            .text_size(px(11.0))
                            .text_color(Theme::global(cx).danger)
                            .hover(move |style| style.bg(glass::hover_bg(cx)).cursor_pointer())
                            .child("清空")
                            .on_click(move |_, window, cx| {
                                clear_view.update(cx, |view, _cx| view.clear_current_history());
                                window.refresh();
                            }),
                    )
                }),
        )
        .child(
            div()
                .id("api-history-scroll")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .scrollbar_width(px(4.0))
                .when(count == 0, |list| {
                    list.child(
                        div()
                            .p(px(12.0))
                            .text_size(px(11.0))
                            .text_color(ui::text_tertiary(cx))
                            .child("暂无历史记录，发送请求后会自动追加"),
                    )
                })
                .children(
                    entries
                        .into_iter()
                        .enumerate()
                        .map(move |(index, entry)| history_row(view.clone(), index, entry, cx)),
                ),
        )
}

fn history_row(
    view: Entity<ApiDebuggerView>,
    index: usize,
    entry: HttpHistory,
    cx: &App,
) -> impl IntoElement {
    let status_color = if entry.status == 0 {
        Theme::global(cx).muted_foreground
    } else if (200..300).contains(&entry.status) {
        Theme::global(cx).success
    } else {
        Theme::global(cx).danger
    };
    div()
        .id(("api-history-row", index))
        .px(px(10.0))
        .py(px(8.0))
        .border_b_1()
        .border_color(glass::divider(cx))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .hover(move |style| style.bg(glass::hover_bg(cx)).cursor_pointer())
        .on_click(move |_, window, cx| {
            view.update(cx, |view, _cx| view.view_history_entry(index));
            window.refresh();
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family("SF Mono")
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(Theme::global(cx).foreground)
                        .child(entry.method.clone()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family("SF Mono")
                        .text_color(status_color)
                        .child(entry.status.to_string()),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(ui::text_tertiary(cx))
                        .child(entry.created_at.clone()),
                ),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(Theme::global(cx).muted_foreground)
                .child(entry.url.clone()),
        )
}

#[derive(Clone, Copy)]
enum ResponseBodyAction {
    Copy,
    Format,
    Save,
}

fn response_action_button(
    view: Entity<ApiDebuggerView>,
    label: &'static str,
    action: ResponseBodyAction,
    _cx: &App,
) -> impl IntoElement {
    let id_index = match action {
        ResponseBodyAction::Copy => 0usize,
        ResponseBodyAction::Format => 1,
        ResponseBodyAction::Save => 2,
    };
    let icon = match action {
        ResponseBodyAction::Copy => IconName::Copy,
        ResponseBodyAction::Format => IconName::CaseSensitive,
        ResponseBodyAction::Save => IconName::File,
    };
    Button::new(("api-response-action", id_index))
        .ghost()
        .icon(icon)
        .label(label)
        .with_size(Size::XSmall)
        .on_click(move |_, window, cx| {
            view.update(cx, |view, cx| match action {
                ResponseBodyAction::Copy => view.copy_response_body(cx),
                ResponseBodyAction::Format => view.format_response_body(),
                ResponseBodyAction::Save => view.save_response_body(),
            });
            window.refresh();
        })
}

fn is_binary_content_type(content_type: &str) -> bool {
    let ct = content_type.to_ascii_lowercase();
    let ct = ct.split(';').next().unwrap_or("").trim();
    ct.starts_with("image/")
        || ct.starts_with("audio/")
        || ct.starts_with("video/")
        || ct.starts_with("font/")
        || ct == "application/octet-stream"
        || ct == "application/pdf"
        || ct == "application/zip"
        || ct == "application/gzip"
}

use crate::view::ApiDebuggerView;
use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, hsla, px,
};
use gpui_component::theme::Theme;
use qingqi_ui::text_input::TextInput;
use qingqi_ui::{theme, ui, ui::glass};

pub fn curl_import_dialog(
    view: Entity<ApiDebuggerView>,
    curl_import_input: Entity<TextInput>,
    cx: &App,
) -> impl IntoElement {
    let import_view = view.clone();
    let cancel_view = view.clone();
    div()
        .w(px(560.0))
        .rounded(px(16.0))
        .border_1()
        .border_color(glass::border(cx))
        .bg(glass::bg(cx))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(44.0))
                .px(px(18.0))
                .border_b_1()
                .border_color(ui::border_light(cx))
                .bg(theme::rgba_with_alpha(
                    Theme::global(cx).list.into(),
                    if Theme::global(cx).is_dark() {
                        0.34
                    } else {
                        0.52
                    },
                ))
                .flex()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(Theme::global(cx).muted_foreground)
                        .child("导入 cURL 命令"),
                ),
        )
        .child(
            div()
                .p(px(16.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(Theme::global(cx).muted_foreground)
                        .child("粘贴 cURL 命令以导入请求"),
                )
                .child(
                    div()
                        .id("curl-import-textarea-wrapper")
                        .child(curl_import_input),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.0))
                        .child(
                            div()
                                .id("curl-import-cancel-btn")
                                .px(px(16.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .bg(theme::rgba_with_alpha(
                                    Theme::global(cx).list.into(),
                                    if Theme::global(cx).is_dark() {
                                        0.5
                                    } else {
                                        0.7
                                    },
                                ))
                                .text_xs()
                                .text_color(Theme::global(cx).muted_foreground)
                                .cursor_pointer()
                                .on_click(move |_event, _window, cx| {
                                    cancel_view.update(cx, |view, _cx| {
                                        view.show_curl_import = false;
                                    });
                                })
                                .child("取消"),
                        )
                        .child(
                            div()
                                .id("curl-import-ok-btn")
                                .px(px(16.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .bg(Theme::global(cx).primary)
                                .text_xs()
                                .text_color(Theme::global(cx).foreground)
                                .cursor_pointer()
                                .on_click(move |_event, _window, cx| {
                                    import_view.update(cx, |view, cx| {
                                        view.import_curl(cx);
                                    });
                                })
                                .child("导入"),
                        ),
                ),
        )
}

pub fn rename_dialog(
    view: Entity<ApiDebuggerView>,
    rename_input: Entity<TextInput>,
    cx: &App,
) -> impl IntoElement {
    let confirm_view = view.clone();
    let cancel_view = view.clone();
    div()
        .w(px(420.0))
        .rounded(px(16.0))
        .border_1()
        .border_color(glass::border(cx))
        .bg(glass::bg(cx))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(44.0))
                .px(px(18.0))
                .border_b_1()
                .border_color(ui::border_light(cx))
                .bg(theme::rgba_with_alpha(
                    Theme::global(cx).list.into(),
                    if Theme::global(cx).is_dark() {
                        0.34
                    } else {
                        0.52
                    },
                ))
                .flex()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(Theme::global(cx).muted_foreground)
                        .child("重命名"),
                ),
        )
        .child(
            div()
                .p(px(16.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(Theme::global(cx).muted_foreground)
                        .child("输入新的名称"),
                )
                .child(div().id("api-rename-input-wrapper").child(rename_input))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.0))
                        .child(
                            div()
                                .id("api-rename-cancel-btn")
                                .px(px(16.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .bg(theme::rgba_with_alpha(
                                    Theme::global(cx).list.into(),
                                    if Theme::global(cx).is_dark() {
                                        0.5
                                    } else {
                                        0.7
                                    },
                                ))
                                .text_xs()
                                .text_color(Theme::global(cx).muted_foreground)
                                .cursor_pointer()
                                .on_click(move |_event, _window, cx| {
                                    cancel_view.update(cx, |view, _cx| {
                                        view.show_rename = false;
                                    });
                                })
                                .child("取消"),
                        )
                        .child(
                            div()
                                .id("api-rename-ok-btn")
                                .px(px(16.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .bg(Theme::global(cx).primary)
                                .text_xs()
                                .text_color(Theme::global(cx).foreground)
                                .cursor_pointer()
                                .on_click(move |_event, _window, cx| {
                                    confirm_view.update(cx, |view, cx| {
                                        view.confirm_rename(cx);
                                    });
                                })
                                .child("确定"),
                        ),
                ),
        )
}

pub fn overlay_shell(
    cx: &App,
    backdrop_id: &'static str,
    on_close: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
    content: impl IntoElement,
) -> impl IntoElement {
    div()
        .size_full()
        .absolute()
        .top_0()
        .left_0()
        .child(
            div()
                .id(backdrop_id)
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .bg(hsla(
                    0.0,
                    0.0,
                    0.0,
                    if Theme::global(cx).is_dark() {
                        0.46
                    } else {
                        0.24
                    },
                ))
                .on_click(move |event, _window, cx| on_close(event, cx)),
        )
        .child(
            div()
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .id("api-overlay-content")
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(content),
                ),
        )
}

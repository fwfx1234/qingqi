use gpui_component::theme::Theme;

use super::shared::{pill_button, toggle_control};
use super::*;

pub(super) fn settings_panel(
    handle: Entity<ClipboardView>,
    config: ClipboardConfig,
    ignore_patterns_input: Entity<TextInput>,
    max_text_chars_input: Entity<TextInput>,
    hotkey_input: Entity<TextInput>,
    cx: &App,
) -> impl IntoElement {
    let app = cx;

    div()
        .size_full()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .p(px(20.0))
        .child(settings_card(
            vec![
                settings_toggle_row(
                    "文本采集",
                    if config.capture_text {
                        "当前开启，新的文本会写入历史"
                    } else {
                        "当前关闭，新的文本不会进入历史"
                    },
                    config.capture_text,
                    {
                        let handle = handle.clone();
                        move |cx| {
                            let _ = cx.update_entity(&handle, |panel, cx| {
                                panel.toggle_capture_text(cx);
                                cx.notify();
                            });
                        }
                    },
                    app,
                ),
                settings_toggle_row(
                    "图片采集",
                    if config.capture_image {
                        "当前开启，后续图片剪贴板将进入历史"
                    } else {
                        "当前关闭，后续图片剪贴板会被跳过"
                    },
                    config.capture_image,
                    {
                        let handle = handle.clone();
                        move |cx| {
                            let _ = cx.update_entity(&handle, |panel, cx| {
                                panel.toggle_capture_image(cx);
                                cx.notify();
                            });
                        }
                    },
                    app,
                ),
                settings_toggle_row(
                    "文件采集",
                    if config.capture_files {
                        "当前开启，后续文件剪贴板将进入历史"
                    } else {
                        "当前关闭，后续文件剪贴板会被跳过"
                    },
                    config.capture_files,
                    {
                        let handle = handle.clone();
                        move |cx| {
                            let _ = cx.update_entity(&handle, |panel, cx| {
                                panel.toggle_capture_files(cx);
                                cx.notify();
                            });
                        }
                    },
                    app,
                ),
            ],
            app,
        ))
        .child(settings_card(
            vec![
                settings_input_row(
                    "文本长度上限",
                    format!("当前上限 {} 字符，超过后跳过采集", config.max_text_chars),
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .w(px(120.0))
                                .child(input_shell(max_text_chars_input, app)),
                        )
                        .child(pill_button("保存", app, {
                            let handle = handle.clone();
                            move |_, cx| {
                                let _ = cx.update_entity(&handle, |panel, cx| {
                                    panel.save_max_text_chars(cx);
                                    cx.notify();
                                });
                            }
                        }))
                        .child(pill_button("4k", app, {
                            let handle = handle.clone();
                            move |_, cx| {
                                let _ = cx.update_entity(&handle, |panel, cx| {
                                    panel.set_max_text_chars(4_096, cx);
                                    cx.notify();
                                });
                            }
                        }))
                        .child(pill_button("20k", app, {
                            let handle = handle.clone();
                            move |_, cx| {
                                let _ = cx.update_entity(&handle, |panel, cx| {
                                    panel.set_max_text_chars(20_000, cx);
                                    cx.notify();
                                });
                            }
                        }))
                        .child(pill_button("100k", app, {
                            let handle = handle.clone();
                            move |_, cx| {
                                let _ = cx.update_entity(&handle, |panel, cx| {
                                    panel.set_max_text_chars(100_000, cx);
                                    cx.notify();
                                });
                            }
                        })),
                    app,
                ),
                settings_input_row(
                    "过滤规则",
                    format!(
                        "当前 {} 条规则；命中内容在采集阶段跳过，支持正则",
                        config.ignore_patterns.len()
                    ),
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .w(px(200.0))
                                .child(input_shell(ignore_patterns_input, app)),
                        )
                        .child(pill_button("保存规则", app, {
                            let handle = handle.clone();
                            move |_, cx| {
                                let _ = cx.update_entity(&handle, |panel, cx| {
                                    panel.save_ignore_patterns(cx);
                                    cx.notify();
                                });
                            }
                        }))
                        .child(pill_button("清空规则", app, {
                            let handle = handle.clone();
                            move |_, cx| {
                                let _ = cx.update_entity(&handle, |panel, cx| {
                                    panel.clear_ignore_patterns(cx);
                                    cx.notify();
                                });
                            }
                        })),
                    app,
                ),
            ],
            app,
        ))
        .child(settings_card(
            vec![settings_input_row(
                "打开快捷键",
                format!("当前保存为 {}；保存后立即重新注册", config.hotkey),
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().w(px(140.0)).child(input_shell(hotkey_input, app)))
                    .child(pill_button("保存快捷键", app, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |panel, cx| {
                                panel.save_hotkey(cx);
                                cx.notify();
                            });
                        }
                    })),
                app,
            )],
            app,
        ))
}

fn settings_card(rows: Vec<gpui::AnyElement>, cx: &App) -> impl IntoElement {
    let t = Theme::global(cx);
    div()
        .rounded(theme::radius_lg())
        .bg(t.list)
        .border_1()
        .border_color(ui::border_light(cx))
        .flex()
        .flex_col()
        .children(rows)
}

fn settings_toggle_row(
    label: &'static str,
    description: impl Into<String>,
    enabled: bool,
    on_toggle: impl Fn(&mut App) + 'static,
    cx: &App,
) -> gpui::AnyElement {
    let t = Theme::global(cx);

    div()
        .min_h(px(52.0))
        .px(px(16.0))
        .py(px(10.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
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
                        .child(description.into()),
                ),
        )
        .child(toggle_control(label, enabled, on_toggle))
        .into_any_element()
}

fn settings_input_row(
    label: &'static str,
    description: impl Into<String>,
    control: impl IntoElement,
    cx: &App,
) -> gpui::AnyElement {
    let t = Theme::global(cx);

    div()
        .min_h(px(52.0))
        .px(px(16.0))
        .py(px(10.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
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
                        .child(description.into()),
                ),
        )
        .child(div().flex_shrink_0().child(control.into_any_element()))
        .into_any_element()
}

fn input_shell(input: Entity<TextInput>, cx: &App) -> impl IntoElement {
    let t = Theme::global(cx);
    div()
        .rounded(theme::radius_md())
        .border_1()
        .border_color(ui::border_light(cx))
        .bg(t.list)
        .child(input.into_any_element())
}

pub(super) fn format_ignore_patterns(config: &ClipboardConfig) -> String {
    config.ignore_patterns.join("\n")
}

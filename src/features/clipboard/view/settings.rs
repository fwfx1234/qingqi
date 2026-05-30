use super::shared::{header_action_button, theme_button};
use super::*;
use gpui_component::scroll::ScrollableElement;

pub(super) fn settings_page(
    handle: Entity<ClipboardPanel>,
    status_text: String,
    config: ClipboardConfig,
    inputs: (Entity<TextInput>, Entity<TextInput>, Entity<TextInput>),
    dark: bool,
) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(theme::semantic(dark).bg_page)
        .child(settings_header(handle.clone(), status_text, dark))
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scrollbar()
                .p(px(14.0))
                .child(settings_panel(handle, config, inputs, dark)),
        )
}

fn settings_header(
    handle: Entity<ClipboardPanel>,
    status_text: String,
    dark: bool,
) -> impl IntoElement {
    div()
        .h(px(62.0))
        .pl(px(108.0))
        .pr(px(16.0))
        .border_b_1()
        .border_color(theme::semantic(dark).border_default)
        .bg(theme::semantic(dark).bg_page)
        .flex()
        .items_center()
        .gap(px(12.0))
        .child(header_action_button(
            "clipboard-settings-back",
            dark,
            "返回",
            {
                let handle = handle.clone();
                move |_, cx| {
                    let _ = cx.update_entity(&handle, |panel, cx| {
                        panel.set_tab(ClipboardTab::History);
                        cx.notify();
                    });
                }
            },
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme::semantic(dark).text_primary)
                        .child("剪贴板设置"),
                )
                .child(
                    div()
                        .max_w(px(420.0))
                        .text_size(px(11.0))
                        .line_clamp(1)
                        .text_color(theme::semantic(dark).text_secondary)
                        .child(status_text),
                ),
        )
        .child(div().flex_1())
}

fn settings_panel(
    handle: Entity<ClipboardPanel>,
    config: ClipboardConfig,
    inputs: (Entity<TextInput>, Entity<TextInput>, Entity<TextInput>),
    dark: bool,
) -> impl IntoElement {
    let (ignore_patterns_input, max_text_chars_input, hotkey_input) = inputs;

    div()
        .w_full()
        .min_w(px(0.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(theme::semantic(dark).border_default)
        .bg(theme::semantic(dark).bg_surface)
        .p_4()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("采集设置"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme::semantic(dark).text_secondary)
                        .child("设置会直接写入 SQLite，供后台采集和后续热键接管使用"),
                ),
        )
        .child(settings_row(
            "文本采集",
            if config.capture_text {
                "当前开启，新的文本会写入历史"
            } else {
                "当前关闭，新的文本不会进入历史"
            },
            theme_button(if config.capture_text {
                "关闭文本采集"
            } else {
                "开启文本采集"
            }, dark, {
                let handle = handle.clone();
                move |_, cx| {
                    let _ = cx.update_entity(&handle, |panel, cx| {
                        panel.toggle_capture_text(cx);
                        cx.notify();
                    });
                }
            }),
            dark,
        ))
        .child(settings_row(
            "图片采集",
            if config.capture_image {
                "当前开启，后续图片剪贴板将进入历史"
            } else {
                "当前关闭，后续图片剪贴板会被跳过"
            },
            theme_button(if config.capture_image {
                "关闭图片采集"
            } else {
                "开启图片采集"
            }, dark, {
                let handle = handle.clone();
                move |_, cx| {
                    let _ = cx.update_entity(&handle, |panel, cx| {
                        panel.toggle_capture_image(cx);
                        cx.notify();
                    });
                }
            }),
            dark,
        ))
        .child(settings_row(
            "文件采集",
            if config.capture_files {
                "当前开启，后续文件剪贴板将进入历史"
            } else {
                "当前关闭，后续文件剪贴板会被跳过"
            },
            theme_button(if config.capture_files {
                "关闭文件采集"
            } else {
                "开启文件采集"
            }, dark, {
                let handle = handle.clone();
                move |_, cx| {
                    let _ = cx.update_entity(&handle, |panel, cx| {
                        panel.toggle_capture_files(cx);
                        cx.notify();
                    });
                }
            }),
            dark,
        ))
        .child(settings_row(
            "文本长度上限",
            format!("当前上限 {} 字符，超过后跳过采集", config.max_text_chars),
            settings_input_group(
                div()
                    .w(px(180.0))
                    .child(input_shell(max_text_chars_input, dark)),
                div()
                    .flex()
                    .gap_2()
                    .child(theme_button("保存", dark, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |panel, cx| {
                                panel.save_max_text_chars(cx);
                                cx.notify();
                            });
                        }
                    }))
                    .child(theme_button("4k", dark, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |panel, cx| {
                                panel.set_max_text_chars(4_096, cx);
                                cx.notify();
                            });
                        }
                    }))
                    .child(theme_button("20k", dark, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |panel, cx| {
                                panel.set_max_text_chars(20_000, cx);
                                cx.notify();
                            });
                        }
                    }))
                    .child(theme_button("100k", dark, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |panel, cx| {
                                panel.set_max_text_chars(100_000, cx);
                                cx.notify();
                            });
                        }
                    })),
            ),
            dark,
        ))
        .child(settings_row(
            "过滤规则",
            format!(
                "当前 {} 条规则；命中内容会在采集阶段被跳过，支持正则，失败时退回大小写不敏感子串匹配",
                config.ignore_patterns.len()
            ),
            settings_input_group(
                div()
                    .w(px(320.0))
                    .child(input_shell(ignore_patterns_input, dark)),
                div()
                    .flex()
                    .gap_2()
                    .child(theme_button("保存规则", dark, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |panel, cx| {
                                panel.save_ignore_patterns(cx);
                                cx.notify();
                            });
                        }
                    }))
                    .child(theme_button("清空规则", dark, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |panel, cx| {
                                panel.clear_ignore_patterns(cx);
                                cx.notify();
                            });
                        }
                    })),
            ),
            dark,
        ))
        .child(settings_row(
            "打开快捷键",
            format!("当前保存为 {}；保存后立即重新注册", config.hotkey),
            settings_input_group(
                div()
                    .w(px(180.0))
                    .child(input_shell(hotkey_input, dark)),
                div().child(theme_button("保存快捷键", dark, {
                    let handle = handle.clone();
                    move |_, cx| {
                        let _ = cx.update_entity(&handle, |panel, cx| {
                            panel.save_hotkey(cx);
                            cx.notify();
                        });
                    }
                })),
            ),
            dark,
        ))
}

fn settings_row(
    title: &'static str,
    detail: impl Into<String>,
    action: impl IntoElement,
    dark: bool,
) -> impl IntoElement {
    div()
        .p_3()
        .rounded(px(6.0))
        .border_1()
        .border_color(theme::semantic(dark).border_default)
        .bg(theme::semantic(dark).bg_page)
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .overflow_hidden()
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .line_clamp(2)
                        .text_color(theme::semantic(dark).text_secondary)
                        .child(detail.into()),
                ),
        )
        .child(div().flex_shrink_0().child(action))
}

fn settings_input_group(field: impl IntoElement, actions: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_2()
        .child(field)
        .child(actions)
}

fn input_shell(input: Entity<TextInput>, dark: bool) -> impl IntoElement {
    div()
        .rounded(px(6.0))
        .border_1()
        .border_color(theme::semantic(dark).border_default)
        .bg(theme::semantic(dark).bg_surface)
        .child(input.into_any_element())
}

pub(super) fn format_ignore_patterns(config: &ClipboardConfig) -> String {
    config.ignore_patterns.join("\n")
}

use super::shared::theme_button;
use super::*;
use gpui_component::scroll::ScrollableElement;
use gpui_component::{Icon, IconName, Sizable, Size as ComponentSize};

/// macOS 风格标题栏：左箭头 + 标题，融入系统窗口 chrome。
pub(super) fn settings_titlebar_slot(
    handle: Entity<ClipboardView>,
    _dark: bool,
) -> impl IntoElement {
    let back_handle = handle.clone();
    div()
        .h_full()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .id("clipboard-settings-back")
                .size(px(26.0))
                .rounded(px(5.0))
                .flex()
                .items_center()
                .justify_center()
                .hover(|style| style.bg(theme::semantic().bg_hover).cursor_pointer())
                .child(
                    Icon::new(IconName::ChevronLeft)
                        .with_size(ComponentSize::Small)
                        .text_color(theme::semantic().text_secondary),
                )
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = cx.update_entity(&back_handle, |panel, cx| {
                        panel.set_tab(ClipboardTab::History);
                        cx.notify();
                    });
                }),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::semantic().text_primary)
                .child("剪贴板设置"),
        )
}

pub(super) fn settings_page(
    handle: Entity<ClipboardView>,
    _status_text: String,
    config: ClipboardConfig,
    inputs: (Entity<TextInput>, Entity<TextInput>, Entity<TextInput>),
    dark: bool,
    chrome_metrics: ui::WindowChromeMetrics,
) -> impl IntoElement {
    let back_handle = handle.clone();

    div()
        .size_full()
        .pt(px(chrome_metrics.content_top_padding))
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .flex_none()
                .px(px(8.0))
                .pt(px(8.0))
                .child(settings_back_button(back_handle)),
        )
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scrollbar()
                .p(px(8.0))
                .child(settings_panel(handle, config, inputs, dark)),
        )
}

fn settings_back_button(handle: Entity<ClipboardView>) -> impl IntoElement {
    div()
        .id("clipboard-settings-content-back")
        .h(px(28.0))
        .px(px(8.0))
        .rounded(px(5.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(theme::rgba_with_alpha(theme::semantic().bg_surface, 0.55))
        .flex()
        .items_center()
        .gap(px(4.0))
        .text_size(px(10.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme::semantic().text_secondary)
        .hover(|style| style.bg(theme::semantic().bg_hover).cursor_pointer())
        .child(
            Icon::new(IconName::ChevronLeft)
                .with_size(ComponentSize::Small)
                .text_color(theme::semantic().text_secondary),
        )
        .child("返回剪贴板")
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            let _ = cx.update_entity(&handle, |panel, cx| {
                panel.set_tab(ClipboardTab::History);
                cx.notify();
            });
        })
}

fn settings_panel(
    handle: Entity<ClipboardView>,
    config: ClipboardConfig,
    inputs: (Entity<TextInput>, Entity<TextInput>, Entity<TextInput>),
    dark: bool,
) -> impl IntoElement {
    let (ignore_patterns_input, max_text_chars_input, hotkey_input) = inputs;

    div()
        .w_full()
        .min_w(px(0.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(theme::rgba_with_alpha(theme::semantic().bg_surface, 0.5))
        .p_2p5()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("采集设置"),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme::semantic().text_secondary)
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
                    .child(input_shell(max_text_chars_input)),
                div()
                    .flex()
                    .gap_1()
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
                    .child(input_shell(ignore_patterns_input)),
                div()
                    .flex()
                    .gap_1()
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
                    .child(input_shell(hotkey_input)),
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
    _dark: bool,
) -> impl IntoElement {
    div()
        .p_2()
        .rounded(px(4.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(theme::rgba_with_alpha(theme::semantic().bg_surface, 0.4))
        .flex()
        .items_center()
        .justify_between()
        .gap_1p5()
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
                        .text_size(px(11.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .line_height(px(14.0))
                        .line_clamp(2)
                        .text_color(theme::semantic().text_secondary)
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
        .gap_1()
        .child(field)
        .child(actions)
}

fn input_shell(input: Entity<TextInput>) -> impl IntoElement {
    div()
        .rounded(px(4.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(theme::rgba_with_alpha(theme::semantic().bg_surface, 0.5))
        .child(input.into_any_element())
}

pub(super) fn format_ignore_patterns(config: &ClipboardConfig) -> String {
    config.ignore_patterns.join("\n")
}

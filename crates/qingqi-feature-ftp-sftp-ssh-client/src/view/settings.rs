use super::*;
use gpui_component::{Icon, IconName, Sizable, Size as ComponentSize};

use crate::settings::{CursorStyle, TerminalSettings, TerminalTheme};

pub(super) fn settings_button(
    handle: Entity<FtpSftpSshView>,
    _dark: bool,
) -> impl IntoElement {
    let handle_click = handle.clone();
    div()
        .id("ssh-settings-btn")
        .size(px(26.0))
        .rounded(px(5.0))
        .flex()
        .items_center()
        .justify_center()
        .hover(|style| style.bg(theme::semantic().bg_hover).cursor_pointer())
        .child(
            Icon::new(IconName::Settings)
                .with_size(ComponentSize::Small)
                .text_color(theme::semantic().text_secondary),
        )
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            let _ = cx.update_entity(&handle_click, |view, cx| {
                view.toggle_settings_panel(cx);
            });
        })
}

pub(super) fn settings_page(
    handle: Entity<FtpSftpSshView>,
    settings: &TerminalSettings,
    inputs: &SettingsInputs,
    dark: bool,
) -> impl IntoElement {
    let close_handle_for_backdrop = handle.clone();
    let close_handle_for_keys = handle.clone();

    ui::components::overlay_host(
        dark,
        "ssh-settings-backdrop",
        move |_, _, app| {
            let _ = app.update_entity(&close_handle_for_backdrop, |view, cx| {
                view.toggle_settings_panel(cx);
            });
        },
        div()
            .w(px(520.0))
            .max_h(px(560.0))
            .rounded(px(12.0))
            .bg(if dark {
                hsla(220.0 / 360.0, 0.12, 0.14, 1.0)
            } else {
                hsla(220.0 / 360.0, 0.16, 0.985, 1.0)
            })
            .border_1()
            .border_color(theme::rgba_with_alpha(
                theme::semantic().border_default,
                0.12,
            ))
            .shadow(settings_shadow())
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                if event.keystroke.key == "escape" {
                    let _ = cx.update_entity(&close_handle_for_keys, |view, cx| {
                        view.toggle_settings_panel(cx);
                    });
                    cx.stop_propagation();
                }
            })
            .child(
                div()
                    .flex_none()
                    .h(px(44.0))
                    .px(px(16.0))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme::rgba_with_alpha(
                        theme::semantic().border_default,
                        0.12,
                    ))
                    .child(settings_back_button(handle.clone())),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scrollbar()
                    .p(px(16.0))
                    .child(settings_panel(handle, settings, inputs, dark)),
            ),
    )
}

fn settings_back_button(handle: Entity<FtpSftpSshView>) -> impl IntoElement {
    div()
        .id("ssh-settings-content-back")
        .h(px(28.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .bg(hsla(0.0, 0.0, 0.0, 0.0))
        .flex()
        .items_center()
        .gap_1()
        .text_size(px(12.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme::semantic().text_secondary)
        .hover(|style| style.bg(theme::semantic().bg_hover).cursor_pointer())
        .child(
            Icon::new(IconName::ChevronLeft)
                .with_size(ComponentSize::Small)
                .text_color(theme::semantic().text_secondary),
        )
        .child("返回设置")
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            let _ = cx.update_entity(&handle, |view, cx| {
                view.toggle_settings_panel(cx);
            });
        })
}

#[derive(Clone)]
pub(super) struct SettingsInputs {
    pub font_family: Entity<TextInput>,
    pub font_size: Entity<TextInput>,
    pub line_height: Entity<TextInput>,
    pub scrollback_lines: Entity<TextInput>,
    pub word_separators: Entity<TextInput>,
}

fn settings_panel(
    handle: Entity<FtpSftpSshView>,
    settings: &TerminalSettings,
    inputs: &SettingsInputs,
    dark: bool,
) -> impl IntoElement {
    let SettingsInputs {
        font_family,
        font_size,
        line_height,
        scrollback_lines,
        word_separators,
    } = inputs;

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
                        .child("终端设置"),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme::semantic().text_secondary)
                        .child("自定义终端字体、主题和交互行为"),
                ),
        )
        .child(settings_row(
            "字体",
            format!("当前字体: {}", settings.font_family),
            settings_input_group(
                div().w(px(200.0)).child(input_shell(font_family.clone(), dark)),
                div().child(theme_button("保存", dark, false, {
                    let handle = handle.clone();
                    move |_, cx| {
                        let _ = cx.update_entity(&handle, |view, cx| {
                            view.save_font_family(cx);
                        });
                    }
                })),
            ),
            dark,
        ))
        .child(settings_row(
            "字体大小",
            format!("当前: {}px", settings.font_size),
            settings_input_group(
                div().w(px(120.0)).child(input_shell(font_size.clone(), dark)),
                div()
                    .flex()
                    .gap_1()
                    .child(theme_button("保存", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.save_font_size(cx);
                            });
                        }
                    }))
                    .child(theme_button("11", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.set_font_size(11.0, cx);
                            });
                        }
                    }))
                    .child(theme_button("13", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.set_font_size(13.0, cx);
                            });
                        }
                    }))
                    .child(theme_button("15", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.set_font_size(15.0, cx);
                            });
                        }
                    }))
                    .child(theme_button("17", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.set_font_size(17.0, cx);
                            });
                        }
                    })),
            ),
            dark,
        ))
        .child(settings_row(
            "行高",
            format!("当前: {}px", settings.line_height),
            settings_input_group(
                div().w(px(120.0)).child(input_shell(line_height.clone(), dark)),
                div()
                    .flex()
                    .gap_1()
                    .child(theme_button("保存", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.save_line_height(cx);
                            });
                        }
                    }))
                    .child(theme_button("16", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.set_line_height(16.0, cx);
                            });
                        }
                    }))
                    .child(theme_button("18", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.set_line_height(18.0, cx);
                            });
                        }
                    }))
                    .child(theme_button("20", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.set_line_height(20.0, cx);
                            });
                        }
                    }))
                    .child(theme_button("22", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.set_line_height(22.0, cx);
                            });
                        }
                    })),
            ),
            dark,
        ))
        .child(settings_row(
            "主题",
            format!("当前: {}", theme_display_name(&settings.theme)),
            theme_selector(handle.clone(), settings.theme.clone(), dark),
            dark,
        ))
        .child(settings_row(
            "光标样式",
            format!("当前: {}", cursor_display_name(&settings.cursor_style)),
            cursor_selector(handle.clone(), settings.cursor_style.clone(), dark),
            dark,
        ))
        .child(settings_row(
            "光标闪烁",
            if settings.blink_cursor {
                "当前开启，光标会闪烁"
            } else {
                "当前关闭，光标保持常亮"
            },
            theme_button(if settings.blink_cursor { "关闭闪烁" } else { "开启闪烁" }, dark, false, {
                let handle = handle.clone();
                move |_, cx| {
                    let _ = cx.update_entity(&handle, |view, cx| {
                        view.toggle_blink_cursor(cx);
                    });
                }
            }),
            dark,
        ))
        .child(settings_row(
            "回滚行数",
            format!("当前: {} 行", settings.scrollback_lines),
            settings_input_group(
                div().w(px(140.0)).child(input_shell(scrollback_lines.clone(), dark)),
                div()
                    .flex()
                    .gap_1()
                    .child(theme_button("保存", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.save_scrollback_lines(cx);
                            });
                        }
                    }))
                    .child(theme_button("2.5k", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.set_scrollback_lines(2500, cx);
                            });
                        }
                    }))
                    .child(theme_button("5k", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.set_scrollback_lines(5000, cx);
                            });
                        }
                    }))
                    .child(theme_button("10k", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.set_scrollback_lines(10000, cx);
                            });
                        }
                    })),
            ),
            dark,
        ))
        .child(settings_row(
            "单词分隔符",
            format!("当前: {}", settings.word_separators),
            settings_input_group(
                div().w(px(240.0)).child(input_shell(word_separators.clone(), dark)),
                div().child(theme_button("保存", dark, false, {
                    let handle = handle.clone();
                    move |_, cx| {
                        let _ = cx.update_entity(&handle, |view, cx| {
                            view.save_word_separators(cx);
                        });
                    }
                })),
            ),
            dark,
        ))
        .child(
            div()
                .p_2()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    theme_button("恢复默认", dark, false, {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.reset_terminal_settings(cx);
                            });
                        }
                    }),
                ),
        )
}

fn theme_selector(
    handle: Entity<FtpSftpSshView>,
    current: TerminalTheme,
    dark: bool,
) -> impl IntoElement {
    let themes = vec![
        (TerminalTheme::OneLight, "One Light"),
        (TerminalTheme::OneDark, "One Dark"),
        (TerminalTheme::SolarizedLight, "Solarized Light"),
        (TerminalTheme::SolarizedDark, "Solarized Dark"),
    ];

    div()
        .flex()
        .flex_wrap()
        .gap_1()
        .children(themes.into_iter().map(move |(theme, label)| {
            let selected = current == theme;
            let handle = handle.clone();
            theme_button(
                label,
                dark,
                selected,
                move |_, cx| {
                    let _ = cx.update_entity(&handle, |view, cx| {
                        view.set_terminal_theme(theme.clone(), cx);
                    });
                },
            )
        }))
}

fn cursor_selector(
    handle: Entity<FtpSftpSshView>,
    current: CursorStyle,
    dark: bool,
) -> impl IntoElement {
    let styles = vec![
        (CursorStyle::Block, "块状"),
        (CursorStyle::Beam, "竖线"),
        (CursorStyle::Underline, "下划线"),
    ];

    div()
        .flex()
        .gap_1()
        .children(styles.into_iter().map(move |(style, label)| {
            let selected = current == style;
            let handle = handle.clone();
            theme_button(
                label,
                dark,
                selected,
                move |_, cx| {
                    let _ = cx.update_entity(&handle, |view, cx| {
                        view.set_cursor_style(style.clone(), cx);
                    });
                },
            )
        }))
}

fn theme_display_name(theme: &TerminalTheme) -> &'static str {
    match theme {
        TerminalTheme::OneLight => "One Light",
        TerminalTheme::OneDark => "One Dark",
        TerminalTheme::SolarizedLight => "Solarized Light",
        TerminalTheme::SolarizedDark => "Solarized Dark",
    }
}

fn cursor_display_name(cursor: &CursorStyle) -> &'static str {
    match cursor {
        CursorStyle::Block => "块状",
        CursorStyle::Beam => "竖线",
        CursorStyle::Underline => "下划线",
    }
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

fn settings_shadow() -> Vec<gpui::BoxShadow> {
    vec![gpui::BoxShadow {
        color: hsla(0.0, 0.0, 0.0, 0.18),
        offset: gpui::point(px(0.0), px(8.0)),
        blur_radius: px(24.0),
        spread_radius: px(0.0),
    }]
}

fn input_shell(input: Entity<InputState>, _dark: bool) -> impl IntoElement {
    div()
        .h(px(34.0))
        .rounded(px(6.0))
        .overflow_hidden()
        .child(Input::new(&input).w_full())
}

fn theme_button(
    label: &'static str,
    _dark: bool,
    selected: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h(px(28.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .bg(if selected {
            ui::accent_color(PluginAccent::Purple).into()
        } else {
            theme::rgba_with_alpha(theme::semantic().bg_surface, 0.55)
        })
        .border_1()
        .border_color(if selected {
            ui::accent_color(PluginAccent::Purple).into()
        } else {
            ui::border_light()
        })
        .flex()
        .items_center()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(if selected {
            rgb(0xffffff)
        } else {
            theme::semantic().text_secondary
        })
        .hover(|style| {
            if selected {
                style
            } else {
                style.bg(theme::semantic().bg_hover).cursor_pointer()
            }
        })
        .child(label)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

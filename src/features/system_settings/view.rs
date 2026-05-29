use std::{cell::RefCell, rc::Rc};

use gpui::{
    AnyElement, App, Component, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::{
    app::{text_input::TextInput, theme, theme_store::ThemeMode},
    core::shortcut::{CORE_PLUGIN_ID, ShortcutScope, ShortcutView},
    features::{app_launcher::service::AppIndexSnapshot, system_settings::plugin::SettingsPanel},
    platform::macos::PermissionStatus,
};

pub struct SettingsElement {
    pub panel: Rc<RefCell<SettingsPanel>>,
}

impl IntoElement for SettingsElement {
    type Element = Component<Self>;

    fn into_element(self) -> Self::Element {
        Component::new(self)
    }
}

impl RenderOnce for SettingsElement {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let panel = self.panel.borrow();
        let dark = crate::app::theme_mode::is_dark();
        let message = panel.message.clone();
        let current_mode = panel.current_mode();
        let system_dark = panel.system_dark();
        let config_path = panel.theme_config_path();

        let retention_seconds = panel.retention_seconds();
        let retention_status = panel.retention_status();
        let retention_message = panel.retention_message_text().to_string();

        let app_index_available = panel.app_index_available();
        let app_snapshot: Option<AppIndexSnapshot> = panel.app_index_snapshot();

        let data_dir = panel.data_dir_path();
        let config_dir = panel.config_dir_path();
        let log_dir = panel.log_dir_path();

        let accessibility_status = panel.accessibility_status();
        let accessibility_text = panel.accessibility_status_text();

        let imported_plugin_root = panel.imported_plugin_root_path();

        let icon_cache_dir = panel.icon_cache_dir_path();
        let icon_cache_message = panel.icon_cache_message_text().to_string();

        let has_app_snapshot = app_index_available && app_snapshot.is_some();
        drop(panel);
        let (shortcut_rows, shortcut_message) = {
            let mut panel = self.panel.borrow_mut();
            let rows = panel.shortcut_rows(cx);
            let message = panel.shortcut_message_text().to_string();
            (rows, message)
        };

        let header_message = if message.is_empty() {
            String::from("主题、窗口保留、应用索引与诊断信息")
        } else {
            message
        };

        let page_bg = theme::token("color-bg-page", dark);
        let text_primary = theme::token("color-text-primary", dark);
        let text_secondary = theme::token("color-text-secondary", dark);

        div()
            .size_full()
            .bg(page_bg)
            .font_family("PingFang SC")
            .text_color(text_primary)
            .flex()
            .flex_col()
            .p(theme::space_4())
            .gap(theme::space_4())
            // ── Header ──
            .child(
                div().flex().items_center().justify_between().child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_size(theme::font_size_title())
                                .font_weight(gpui::FontWeight::BOLD)
                                .child("系统设置"),
                        )
                        .child(
                            div()
                                .text_size(theme::font_size_caption())
                                .text_color(text_secondary)
                                .child(header_message),
                        ),
                ),
            )
            // ── Theme & Appearance ──
            .child(settings_card(
                dark,
                "主题与外观",
                Some("控制台视觉样式"),
                div()
                    .flex()
                    .flex_col()
                    .child(settings_row(
                        dark,
                        "主题模式",
                        "切换浅色 / 深色 / 跟随系统外观",
                        mode_segment(Rc::clone(&self.panel), current_mode, dark),
                    ))
                    .child(settings_row(
                        dark,
                        "系统检测",
                        if system_dark {
                            "当前系统外观: 深色"
                        } else {
                            "当前系统外观: 浅色"
                        },
                        div()
                            .h(px(24.0))
                            .px_2()
                            .rounded(px(999.0))
                            .bg(theme::token("color-bg-subtle", dark))
                            .flex()
                            .items_center()
                            .text_size(theme::font_size_caption())
                            .text_color(text_secondary)
                            .child(if system_dark { "深色" } else { "浅色" }),
                    )),
            ))
            // ── Plugin Retention ──
            .child(settings_card(
                dark,
                "插件管理",
                Some("窗口保留与导入管理"),
                div()
                    .flex()
                    .flex_col()
                    .child(settings_row(
                        dark,
                        "插件窗口保留",
                        &retention_status,
                        retention_control(
                            Rc::clone(&self.panel),
                            retention_seconds,
                            retention_message,
                            dark,
                        ),
                    ))
                    .child(settings_row(
                        dark,
                        "导入插件",
                        "目录/ZIP 导入尚未实现；可打开目标目录查看",
                        plugin_dir_button(Rc::clone(&self.panel), dark, &imported_plugin_root),
                    ))
                    .child(settings_row(
                        dark,
                        "已安装插件管理",
                        "管理已安装插件的启用/卸载",
                        disabled_badge(dark, "尚未实现"),
                    )),
            ))
            // ── Shortcuts ──
            .child(settings_card(
                dark,
                "快捷键",
                Some("全局与应用内快捷键"),
                shortcuts_section(
                    Rc::clone(&self.panel),
                    shortcut_rows,
                    shortcut_message,
                    dark,
                ),
            ))
            // ── App Index ──
            .child(settings_card(
                dark,
                "应用索引",
                Some("软件快速启动的应用缓存"),
                div().flex().flex_col().child(app_index_row(
                    Rc::clone(&self.panel),
                    dark,
                    has_app_snapshot,
                    app_snapshot,
                )),
            ))
            // ── macOS Permissions ──
            .child(settings_card(
                dark,
                "macOS 权限",
                Some("系统级访问授权状态"),
                div()
                    .flex()
                    .flex_col()
                    .child(accessibility_row(
                        Rc::clone(&self.panel),
                        dark,
                        accessibility_status,
                        &accessibility_text,
                    ))
                    .child(permission_row(
                        dark,
                        "剪贴板访问",
                        "读取系统剪贴板内容",
                        PermissionStatus::Unknown,
                    ))
                    .child(permission_row(
                        dark,
                        "文件访问",
                        "读取用户目录与应用目录",
                        PermissionStatus::Unknown,
                    ))
                    .child(permission_row(
                        dark,
                        "屏幕录制",
                        "截图、取色等插件可能用到",
                        PermissionStatus::Unknown,
                    )),
            ))
            // ── Diagnostics ──
            .child(settings_card(
                dark,
                "开发诊断",
                Some("数据、缓存与日志路径"),
                div()
                    .flex()
                    .flex_col()
                    .child(diag_path_row(
                        Rc::clone(&self.panel),
                        dark,
                        "数据目录",
                        "Qingqi 应用数据根目录",
                        &data_dir,
                        DiagAction::DataDir,
                    ))
                    .child(diag_path_row(
                        Rc::clone(&self.panel),
                        dark,
                        "配置目录",
                        "配置文件与数据库路径",
                        &config_dir,
                        DiagAction::ConfigDir,
                    ))
                    .child(diag_path_row(
                        Rc::clone(&self.panel),
                        dark,
                        "日志目录",
                        "运行日志输出目录",
                        &log_dir,
                        DiagAction::LogDir,
                    ))
                    .child(settings_row(
                        dark,
                        "主题配置",
                        "当前主题持久化文件",
                        path_badge(dark, &config_path),
                    ))
                    .child(settings_row(
                        dark,
                        "应用索引维护",
                        "手动重建软件快速启动的应用索引",
                        app_index_action_button(Rc::clone(&self.panel), dark, has_app_snapshot),
                    ))
                    .child(settings_row(
                        dark,
                        "清理图标缓存",
                        &icon_cache_dir,
                        icon_cache_clear_button(Rc::clone(&self.panel), dark, icon_cache_message),
                    ))
                    .child(settings_row(
                        dark,
                        "日志诊断",
                        "后台服务状态、最近错误、警告统计",
                        disabled_badge(dark, "尚未实现"),
                    )),
            ))
    }
}

// ── Retention control ──

fn retention_control(
    panel: Rc<RefCell<SettingsPanel>>,
    seconds: u64,
    message: String,
    dark: bool,
) -> impl IntoElement {
    let text_primary = theme::token("color-text-primary", dark);
    let text_secondary = theme::token("color-text-secondary", dark);

    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(
            // Decrement button
            div()
                .id("system-settings-retention-decrement")
                .h(px(28.0))
                .w(px(28.0))
                .rounded(theme::radius_sm())
                .border_1()
                .border_color(theme::token("color-border-default", dark))
                .bg(theme::token("color-bg-surface", dark))
                .hover(|style| {
                    style
                        .bg(theme::token("color-bg-subtle", dark))
                        .cursor_pointer()
                })
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_body())
                .text_color(text_primary)
                .child("−")
                .on_click({
                    let panel = Rc::clone(&panel);
                    move |_, window, _cx| {
                        panel.borrow_mut().adjust_retention(-30);
                        window.refresh();
                    }
                }),
        )
        .child(
            // Value display
            div()
                .h(px(28.0))
                .min_w(px(56.0))
                .rounded(theme::radius_sm())
                .bg(theme::token("color-bg-subtle", dark))
                .border_1()
                .border_color(theme::token("color-border-default", dark))
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_body())
                .text_color(text_primary)
                .child(format!("{seconds}秒")),
        )
        .child(
            // Increment button
            div()
                .id("system-settings-retention-increment")
                .h(px(28.0))
                .w(px(28.0))
                .rounded(theme::radius_sm())
                .border_1()
                .border_color(theme::token("color-border-default", dark))
                .bg(theme::token("color-bg-surface", dark))
                .hover(|style| {
                    style
                        .bg(theme::token("color-bg-subtle", dark))
                        .cursor_pointer()
                })
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_body())
                .text_color(text_primary)
                .child("+")
                .on_click({
                    let panel = Rc::clone(&panel);
                    move |_, window, _cx| {
                        panel.borrow_mut().adjust_retention(30);
                        window.refresh();
                    }
                }),
        )
        .child(
            // Save button
            div()
                .id("system-settings-retention-save")
                .h(px(28.0))
                .px_2()
                .ml(px(4.0))
                .rounded(theme::radius_sm())
                .bg(theme::token("color-primary", dark))
                .hover(|style| {
                    style
                        .bg(theme::token("color-primary-hover", dark))
                        .cursor_pointer()
                })
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_caption())
                .text_color(theme::white())
                .child("保存")
                .on_click({
                    let panel = Rc::clone(&panel);
                    move |_, window, _cx| {
                        panel.borrow_mut().save_retention();
                        window.refresh();
                    }
                }),
        )
        .child(
            // Restore default
            div()
                .id("system-settings-retention-default")
                .h(px(28.0))
                .px_2()
                .rounded(theme::radius_sm())
                .border_1()
                .border_color(theme::token("color-border-default", dark))
                .bg(theme::token("color-bg-surface", dark))
                .hover(|style| {
                    style
                        .bg(theme::token("color-bg-subtle", dark))
                        .cursor_pointer()
                })
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_caption())
                .text_color(text_primary)
                .child("默认")
                .on_click({
                    let panel = Rc::clone(&panel);
                    move |_, window, _cx| {
                        panel.borrow_mut().restore_default_retention();
                        window.refresh();
                    }
                }),
        )
        .when(!message.is_empty(), |el| {
            el.child(
                div()
                    .text_size(theme::font_size_caption())
                    .text_color(text_secondary)
                    .ml(px(4.0))
                    .child(message),
            )
        })
}

// ── App index row ──

fn app_index_row(
    panel: Rc<RefCell<SettingsPanel>>,
    dark: bool,
    has_snapshot: bool,
    snapshot: Option<AppIndexSnapshot>,
) -> impl IntoElement {
    let text_secondary = theme::token("color-text-secondary", dark);
    let text_primary = theme::token("color-text-primary", dark);

    let (status_line, show_rescan) = if !has_snapshot {
        (
            String::from("应用索引服务不可用 — 启动 app-launcher 插件后可用"),
            false,
        )
    } else if let Some(ref snap) = snapshot {
        if snap.scan_running {
            if snap.icon_refresh_running {
                (
                    format!("已索引 {} 个应用，正在补全图标", snap.apps.len()),
                    true,
                )
            } else {
                (
                    format!("已缓存 {} 个应用，后台刷新中", snap.apps.len()),
                    true,
                )
            }
        } else if let Some(ref last_scan) = snap.last_scan {
            (
                format!(
                    "已索引 {} 个应用 · 上次扫描: {}",
                    snap.apps.len(),
                    last_scan
                ),
                true,
            )
        } else {
            (format!("已缓存 {} 个应用", snap.apps.len()), true)
        }
    } else {
        (String::from("正在加载应用索引状态"), true)
    };

    let action = if show_rescan {
        action_button(dark, "重扫描", true, {
            move |_, window, _cx| {
                panel.borrow_mut().request_rescan();
                window.refresh();
            }
        })
        .into_any_element()
    } else {
        disabled_badge(dark, "不可用").into_any_element()
    };

    div()
        .min_h(px(52.0))
        .px(theme::space_4())
        .py(theme::space_2())
        .border_b_1()
        .border_color(theme::token("color-border-default", dark))
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
                        .text_color(text_primary)
                        .child("索引状态"),
                )
                .child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(text_secondary)
                        .line_height(px(16.0))
                        .child(status_line),
                ),
        )
        .child(div().flex_shrink_0().child(action))
}

fn app_index_action_button(
    panel: Rc<RefCell<SettingsPanel>>,
    dark: bool,
    available: bool,
) -> AnyElement {
    if available {
        action_button(dark, "重建索引", true, {
            move |_, window, _cx| {
                panel.borrow_mut().request_rescan();
                window.refresh();
            }
        })
        .into_any_element()
    } else {
        disabled_badge(dark, "服务不可用").into_any_element()
    }
}

fn plugin_dir_button(
    panel: Rc<RefCell<SettingsPanel>>,
    dark: bool,
    _root_path: &str,
) -> impl IntoElement {
    div()
        .id("system-settings-open-plugin-dir")
        .h(px(28.0))
        .px_3()
        .rounded(theme::radius_md())
        .border_1()
        .border_color(theme::token("color-border-default", dark))
        .bg(theme::token("color-bg-surface", dark))
        .hover(|style| {
            style
                .bg(theme::token("color-bg-subtle", dark))
                .cursor_pointer()
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::font_size_caption())
        .text_color(theme::token("color-text-primary", dark))
        .child("打开目录")
        .on_click(move |_, window, _cx| {
            panel.borrow_mut().open_plugin_dir();
            window.refresh();
        })
}

fn icon_cache_clear_button(
    panel: Rc<RefCell<SettingsPanel>>,
    dark: bool,
    message: String,
) -> impl IntoElement {
    let text_secondary = theme::token("color-text-secondary", dark);

    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .id("system-settings-clear-icon-cache")
                .h(px(28.0))
                .px_3()
                .rounded(theme::radius_md())
                .bg(theme::token("color-primary", dark))
                .hover(|style| {
                    style
                        .bg(theme::token("color-primary-hover", dark))
                        .cursor_pointer()
                })
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_caption())
                .text_color(theme::white())
                .child("清理缓存")
                .on_click(move |_, window, _cx| {
                    panel.borrow_mut().clear_icon_cache();
                    window.refresh();
                }),
        )
        .when(!message.is_empty(), |el| {
            el.child(
                div()
                    .text_size(theme::font_size_caption())
                    .text_color(text_secondary)
                    .child(message),
            )
        })
}

// ── Shortcuts ──

fn shortcuts_section(
    panel: Rc<RefCell<SettingsPanel>>,
    rows: Vec<(ShortcutView, Entity<TextInput>)>,
    message: String,
    dark: bool,
) -> impl IntoElement {
    let text_secondary = theme::token("color-text-secondary", dark);

    div()
        .flex()
        .flex_col()
        .when(!message.is_empty(), |el| {
            el.child(
                div()
                    .px(theme::space_4())
                    .py(theme::space_2())
                    .border_b_1()
                    .border_color(theme::token("color-border-default", dark))
                    .text_size(theme::font_size_caption())
                    .text_color(text_secondary)
                    .child(message),
            )
        })
        .when(rows.is_empty(), |el| {
            el.child(
                div()
                    .px(theme::space_4())
                    .py(theme::space_3())
                    .text_size(theme::font_size_caption())
                    .text_color(text_secondary)
                    .child("暂无快捷键声明"),
            )
        })
        .children(
            rows.into_iter()
                .map(|(view, input)| shortcut_row(Rc::clone(&panel), view, input, dark)),
        )
}

fn shortcut_row(
    panel: Rc<RefCell<SettingsPanel>>,
    view: ShortcutView,
    input: Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    let text_primary = theme::token("color-text-primary", dark);
    let text_secondary = theme::token("color-text-secondary", dark);
    let descriptor = view.descriptor.clone();
    let scope_label = descriptor.scope.label();
    let owner_label = if descriptor.owner_plugin_id == CORE_PLUGIN_ID {
        String::from("核心")
    } else {
        descriptor.owner_plugin_id.clone()
    };
    let context_label = descriptor
        .context
        .clone()
        .unwrap_or_else(|| String::from("任意上下文"));
    let default_text = if descriptor.default_accelerator.trim().is_empty() {
        String::from("无")
    } else {
        descriptor.default_accelerator.clone()
    };
    let enabled = descriptor.enabled;
    let editable = descriptor.editable;
    let status = shortcut_status(&view);
    let status_style = shortcut_status_style(&view, dark);
    let shortcut_id = descriptor.id.clone();
    let save_enabled = editable;

    div()
        .min_h(px(68.0))
        .px(theme::space_4())
        .py(theme::space_2())
        .border_b_1()
        .border_color(theme::token("color-border-default", dark))
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
                .gap(px(5.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(theme::font_size_body())
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(text_primary)
                                .child(descriptor.title.clone()),
                        )
                        .child(scope_badge(dark, scope_label, descriptor.scope))
                        .child(status_badge(status, status_style)),
                )
                .child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(text_secondary)
                        .line_height(px(16.0))
                        .child(format!(
                            "{owner_label} · {context_label} · 默认 {default_text}"
                        )),
                ),
        )
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(shortcut_input_shell(input.clone(), dark, editable))
                .child(shortcut_action_button(dark, "保存", true, save_enabled, {
                    let panel = Rc::clone(&panel);
                    let shortcut_id = shortcut_id.clone();
                    let input = input.clone();
                    move |_, window, cx| {
                        panel
                            .borrow_mut()
                            .save_shortcut(&shortcut_id, input.clone(), true, cx);
                        window.refresh();
                    }
                }))
                .child(shortcut_action_button(
                    dark,
                    if enabled { "禁用" } else { "启用" },
                    false,
                    editable,
                    {
                        let panel = Rc::clone(&panel);
                        let shortcut_id = shortcut_id.clone();
                        let input = input.clone();
                        move |_, window, cx| {
                            panel.borrow_mut().save_shortcut(
                                &shortcut_id,
                                input.clone(),
                                !enabled,
                                cx,
                            );
                            window.refresh();
                        }
                    },
                ))
                .child(shortcut_action_button(dark, "默认", false, editable, {
                    let panel = Rc::clone(&panel);
                    let shortcut_id = shortcut_id.clone();
                    move |_, window, cx| {
                        panel.borrow_mut().restore_shortcut(&shortcut_id, cx);
                        window.refresh();
                    }
                })),
        )
}

fn shortcut_input_shell(input: Entity<TextInput>, dark: bool, editable: bool) -> impl IntoElement {
    div()
        .w(px(160.0))
        .rounded(theme::radius_sm())
        .border_1()
        .border_color(theme::token("color-border-default", dark))
        .bg(if editable {
            theme::token("color-bg-surface", dark)
        } else {
            theme::token("color-bg-subtle", dark)
        })
        .child(input.into_any_element())
}

fn shortcut_action_button(
    dark: bool,
    label: &'static str,
    primary: bool,
    enabled: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let bg = if !enabled {
        theme::token("color-bg-subtle", dark)
    } else if primary {
        theme::token("color-primary", dark)
    } else {
        theme::token("color-bg-surface", dark)
    };
    let text = if !enabled {
        theme::token("color-text-secondary", dark)
    } else if primary {
        theme::white()
    } else {
        theme::token("color-text-primary", dark)
    };

    div()
        .id(label)
        .h(px(28.0))
        .px_3()
        .rounded(theme::radius_md())
        .bg(bg)
        .border_1()
        .border_color(theme::token("color-border-default", dark))
        .hover(move |style| {
            if enabled {
                style
                    .bg(if primary {
                        theme::token("color-primary-hover", dark)
                    } else {
                        theme::token("color-bg-subtle", dark)
                    })
                    .cursor_pointer()
            } else {
                style
            }
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::font_size_caption())
        .text_color(text)
        .child(label)
        .when(enabled, |button| {
            button.on_click(move |event, window, cx| on_click(event, window, cx))
        })
}

fn shortcut_status(view: &ShortcutView) -> String {
    if let Some(error) = &view.error {
        return error.clone();
    }
    if let Some(owner) = &view.overridden_by {
        return format!("被 {owner} 覆盖");
    }
    if !view.descriptor.enabled {
        return String::from("已禁用");
    }
    if view.active {
        String::from("生效")
    } else {
        String::from("未生效")
    }
}

fn shortcut_status_style(view: &ShortcutView, dark: bool) -> (gpui::Rgba, gpui::Rgba) {
    if view.error.is_some() || view.overridden_by.is_some() {
        return (
            theme::token("color-warning", dark),
            theme::rgba_with_alpha(theme::token("color-warning", dark), 0.1).into(),
        );
    }
    if !view.descriptor.enabled {
        return (
            theme::token("color-text-secondary", dark),
            theme::rgba_with_alpha(theme::token("color-text-secondary", dark), 0.08).into(),
        );
    }
    if view.active {
        return (
            theme::token("color-success", dark),
            theme::rgba_with_alpha(theme::token("color-success", dark), 0.1).into(),
        );
    }
    (
        theme::token("color-text-secondary", dark),
        theme::rgba_with_alpha(theme::token("color-text-secondary", dark), 0.08).into(),
    )
}

fn scope_badge(dark: bool, text: &'static str, scope: ShortcutScope) -> impl IntoElement {
    let color = match scope {
        ShortcutScope::Global => theme::launcher_accent(dark),
        ShortcutScope::App => theme::token("color-text-secondary", dark),
    };
    div()
        .h(px(20.0))
        .px_2()
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(color, 0.1))
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::font_size_caption())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(color)
        .child(text)
}

fn status_badge(text: String, style: (gpui::Rgba, gpui::Rgba)) -> impl IntoElement {
    let (color, bg) = style;
    div()
        .h(px(20.0))
        .px_2()
        .rounded(px(999.0))
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::font_size_caption())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(color)
        .child(text)
}

// ── Shared Layout Helpers ──

fn settings_card(
    dark: bool,
    title: &'static str,
    subtitle: Option<&'static str>,
    content: impl IntoElement,
) -> impl IntoElement {
    div()
        .rounded(theme::radius_lg())
        .border_1()
        .border_color(theme::token("color-border-default", dark))
        .bg(theme::token("color-bg-surface", dark))
        .flex()
        .flex_col()
        .child(
            div()
                .px(theme::space_4())
                .py(theme::space_3())
                .border_b_1()
                .border_color(theme::token("color-border-default", dark))
                .bg(theme::token("color-bg-subtle-2", dark))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_size(theme::font_size_body())
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme::token("color-text-primary", dark))
                                .child(title),
                        )
                        .when(subtitle.is_some(), |el| {
                            el.child(
                                div()
                                    .text_size(theme::font_size_caption())
                                    .text_color(theme::token("color-text-secondary", dark))
                                    .child(subtitle.unwrap_or("")),
                            )
                        }),
                ),
        )
        .child(div().flex().flex_col().child(content))
}

fn settings_row(
    dark: bool,
    label: &'static str,
    description: &str,
    control: impl IntoElement,
) -> impl IntoElement {
    div()
        .min_h(px(52.0))
        .px(theme::space_4())
        .py(theme::space_2())
        .border_b_1()
        .border_color(theme::token("color-border-default", dark))
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
                        .text_color(theme::token("color-text-primary", dark))
                        .child(label),
                )
                .child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(theme::token("color-text-secondary", dark))
                        .line_height(px(16.0))
                        .child(description.to_string()),
                ),
        )
        .child(div().flex_shrink_0().child(control))
}

fn permission_row(
    dark: bool,
    label: &'static str,
    description: &'static str,
    status: PermissionStatus,
) -> impl IntoElement {
    let (status_text, status_color, status_bg) = match status {
        PermissionStatus::Authorized => (
            "已授权",
            theme::token("color-success", dark),
            theme::rgba_with_alpha(theme::token("color-success", dark), 0.1),
        ),
        PermissionStatus::NotAuthorized => (
            "未授权",
            theme::token("color-warning", dark),
            theme::rgba_with_alpha(theme::token("color-warning", dark), 0.1),
        ),
        PermissionStatus::Unknown => (
            "尚未实现",
            theme::token("color-text-secondary", dark),
            theme::rgba_with_alpha(theme::token("color-text-secondary", dark), 0.08),
        ),
    };

    div()
        .min_h(px(52.0))
        .px(theme::space_4())
        .py(theme::space_2())
        .border_b_1()
        .border_color(theme::token("color-border-default", dark))
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
                        .text_color(theme::token("color-text-primary", dark))
                        .child(label),
                )
                .child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(theme::token("color-text-secondary", dark))
                        .line_height(px(16.0))
                        .child(description),
                ),
        )
        .child(
            div()
                .h(px(22.0))
                .px_2()
                .rounded(px(999.0))
                .bg(status_bg)
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_caption())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(status_color)
                .child(status_text),
        )
}

// ── Accessibility row (real status + open settings button) ──

fn accessibility_row(
    panel: Rc<RefCell<SettingsPanel>>,
    dark: bool,
    status: PermissionStatus,
    text: &str,
) -> impl IntoElement {
    let text_secondary = theme::token("color-text-secondary", dark);
    let text_primary = theme::token("color-text-primary", dark);

    let (status_color, status_bg) = match status {
        PermissionStatus::Authorized => (
            theme::token("color-success", dark),
            theme::rgba_with_alpha(theme::token("color-success", dark), 0.1),
        ),
        PermissionStatus::NotAuthorized => (
            theme::token("color-warning", dark),
            theme::rgba_with_alpha(theme::token("color-warning", dark), 0.1),
        ),
        PermissionStatus::Unknown => (
            theme::token("color-text-secondary", dark),
            theme::rgba_with_alpha(theme::token("color-text-secondary", dark), 0.08),
        ),
    };

    div()
        .min_h(px(52.0))
        .px(theme::space_4())
        .py(theme::space_2())
        .border_b_1()
        .border_color(theme::token("color-border-default", dark))
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
                        .text_color(text_primary)
                        .child("辅助功能"),
                )
                .child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(text_secondary)
                        .line_height(px(16.0))
                        .child("全局热键、窗口聚焦需要"),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .h(px(22.0))
                        .px_2()
                        .rounded(px(999.0))
                        .bg(status_bg)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(theme::font_size_caption())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(status_color)
                        .child(text.to_string()),
                )
                .child(
                    div()
                        .id("system-settings-open-accessibility")
                        .h(px(28.0))
                        .px_3()
                        .rounded(theme::radius_md())
                        .bg(theme::token("color-primary", dark))
                        .hover(|style| {
                            style
                                .bg(theme::token("color-primary-hover", dark))
                                .cursor_pointer()
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(theme::font_size_caption())
                        .text_color(theme::white())
                        .child("打开设置")
                        .on_click(move |_, window, _cx| {
                            panel.borrow_mut().open_accessibility_settings();
                            window.refresh();
                        }),
                ),
        )
}

// ── Diagnostics path row with open button ──

#[derive(Clone, Copy)]
enum DiagAction {
    DataDir,
    ConfigDir,
    LogDir,
}

fn diag_path_row(
    panel: Rc<RefCell<SettingsPanel>>,
    dark: bool,
    label: &'static str,
    _description: &'static str,
    path: &str,
    action: DiagAction,
) -> impl IntoElement {
    let id_key: &'static str = match action {
        DiagAction::DataDir => "diag-open-data",
        DiagAction::ConfigDir => "diag-open-config",
        DiagAction::LogDir => "diag-open-log",
    };

    div()
        .min_h(px(52.0))
        .px(theme::space_4())
        .py(theme::space_2())
        .border_b_1()
        .border_color(theme::token("color-border-default", dark))
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
                        .text_color(theme::token("color-text-primary", dark))
                        .child(label),
                )
                .child(path_badge(dark, path)),
        )
        .child(
            div()
                .id(id_key)
                .h(px(28.0))
                .px_3()
                .rounded(theme::radius_md())
                .border_1()
                .border_color(theme::token("color-border-default", dark))
                .bg(theme::token("color-bg-surface", dark))
                .hover(|style| {
                    style
                        .bg(theme::token("color-bg-subtle", dark))
                        .cursor_pointer()
                })
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_caption())
                .text_color(theme::token("color-text-primary", dark))
                .child("打开")
                .on_click(move |_, window, _cx| {
                    match action {
                        DiagAction::DataDir => panel.borrow_mut().open_data_dir(),
                        DiagAction::ConfigDir => panel.borrow_mut().open_config_dir(),
                        DiagAction::LogDir => panel.borrow_mut().open_log_dir(),
                    }
                    window.refresh();
                }),
        )
}

fn disabled_badge(dark: bool, text: &'static str) -> impl IntoElement {
    let status_color = theme::token("color-text-secondary", dark);
    let status_bg = theme::rgba_with_alpha(theme::token("color-text-secondary", dark), 0.08);

    div()
        .h(px(28.0))
        .px_3()
        .rounded(px(999.0))
        .bg(status_bg)
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::font_size_caption())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(status_color)
        .child(text)
}

fn path_badge(dark: bool, path: &str) -> impl IntoElement {
    div()
        .h(px(28.0))
        .px_2()
        .rounded(theme::radius_sm())
        .bg(theme::token("color-bg-subtle", dark))
        .border_1()
        .border_color(theme::token("color-border-default", dark))
        .flex()
        .items_center()
        .font_family("SF Mono")
        .text_size(theme::font_size_caption())
        .text_color(theme::token("color-text-secondary", dark))
        .child(path.to_string())
}

fn action_button(
    dark: bool,
    label: &'static str,
    primary: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    if primary {
        div()
            .id(label)
            .h(px(28.0))
            .px_3()
            .rounded(theme::radius_md())
            .bg(theme::token("color-primary", dark))
            .hover(|style| {
                style
                    .bg(theme::token("color-primary-hover", dark))
                    .cursor_pointer()
            })
            .flex()
            .items_center()
            .justify_center()
            .text_size(theme::font_size_caption())
            .text_color(theme::white())
            .child(label)
            .on_click(move |event, window, cx| on_click(event, window, cx))
    } else {
        div()
            .id(label)
            .h(px(28.0))
            .px_3()
            .rounded(theme::radius_md())
            .bg(theme::token("color-bg-surface", dark))
            .border_1()
            .border_color(theme::token("color-border-default", dark))
            .hover(|style| {
                style
                    .bg(theme::token("color-bg-subtle", dark))
                    .cursor_pointer()
            })
            .flex()
            .items_center()
            .justify_center()
            .text_size(theme::font_size_caption())
            .text_color(theme::token("color-text-primary", dark))
            .child(label)
            .on_click(move |event, window, cx| on_click(event, window, cx))
    }
}

// ── Segmented Control for Theme Mode ──

fn mode_segment(
    panel: Rc<RefCell<SettingsPanel>>,
    current_mode: ThemeMode,
    dark: bool,
) -> impl IntoElement {
    div()
        .flex()
        .gap(px(2.0))
        .p(px(2.0))
        .rounded(theme::radius_md())
        .border_1()
        .border_color(theme::token("color-border-default", dark))
        .bg(theme::token("color-bg-subtle", dark))
        .child(seg_button(
            Rc::clone(&panel),
            ThemeMode::Light,
            current_mode,
            dark,
        ))
        .child(seg_button(
            Rc::clone(&panel),
            ThemeMode::Dark,
            current_mode,
            dark,
        ))
        .child(seg_button(
            Rc::clone(&panel),
            ThemeMode::System,
            current_mode,
            dark,
        ))
}

fn seg_button(
    panel: Rc<RefCell<SettingsPanel>>,
    mode: ThemeMode,
    current_mode: ThemeMode,
    dark: bool,
) -> impl IntoElement {
    let active = current_mode == mode;
    let text_color = if active {
        theme::token("color-primary", dark)
    } else {
        theme::token("color-text-secondary", dark)
    };

    let mut btn = div()
        .id(mode.persisted_value())
        .h(px(26.0))
        .px_3()
        .rounded(theme::radius_sm())
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::font_size_caption())
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(text_color)
        .child(mode_short_label(mode))
        .hover(move |style| {
            style
                .bg(theme::token("color-bg-surface", dark))
                .cursor_pointer()
        })
        .on_click(move |_, window, _cx| {
            panel.borrow_mut().set_theme_mode(mode);
            window.refresh();
        });

    if active {
        btn = btn
            .bg(theme::token("color-bg-surface", dark))
            .border_1()
            .border_color(theme::token("color-primary-soft", dark))
            .shadow(vec![gpui::BoxShadow {
                color: theme::rgba_with_alpha(theme::token("color-shadow", dark), 0.06),
                offset: gpui::point(px(0.0), px(2.0)),
                blur_radius: px(6.0),
                spread_radius: px(0.0),
            }]);
    }

    btn
}

fn mode_short_label(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Light => "浅色",
        ThemeMode::Dark => "深色",
        ThemeMode::System => "跟随系统",
    }
}

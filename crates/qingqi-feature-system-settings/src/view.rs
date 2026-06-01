use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gpui::{
    AnyElement, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use qingqi_platform::macos::PermissionStatus;
use qingqi_plugin::{
    app::AppIndexSnapshot,
    host::{AppIndexHandleRef, ShortcutHandleRef, ThemeHandleRef},
    shortcut::{CORE_PLUGIN_ID, ShortcutScope, ShortcutView},
    storage::AppPaths,
    theme::ThemeMode,
};
use qingqi_ui::{
    text_input::{TextInput, TextInputStyle},
    theme,
    ui::{self, components},
};

use crate::settings_store::{SettingsStore, retention_status_text};

// ── SettingsView (model + rendering) ──

pub struct SettingsView {
    theme_handle: ThemeHandleRef,
    settings_store: Arc<Mutex<SettingsStore>>,
    app_index_handle: Option<AppIndexHandleRef>,
    shortcut_handle: Option<ShortcutHandleRef>,
    app_paths: AppPaths,
    pub message: String,
    retention_draft: u64,
    retention_message: String,
    accessibility_status: PermissionStatus,
    icon_cache_message: String,
    shortcut_inputs: HashMap<String, Entity<TextInput>>,
    shortcut_drafts: HashMap<String, String>,
    shortcut_message: String,
}

impl SettingsView {
    pub fn new(
        theme_handle: ThemeHandleRef,
        settings_store: Arc<Mutex<SettingsStore>>,
        app_index_handle: Option<AppIndexHandleRef>,
        shortcut_handle: Option<ShortcutHandleRef>,
        app_paths: AppPaths,
    ) -> Self {
        let retention = settings_store
            .lock()
            .ok()
            .map(|store| store.plugin_window_retention_seconds())
            .unwrap_or(300);
        let accessibility_status = qingqi_platform::macos::check_accessibility();
        Self {
            theme_handle,
            settings_store,
            app_index_handle,
            shortcut_handle,
            app_paths,
            message: String::new(),
            retention_draft: retention,
            retention_message: String::new(),
            accessibility_status,
            icon_cache_message: String::new(),
            shortcut_inputs: HashMap::new(),
            shortcut_drafts: HashMap::new(),
            shortcut_message: String::new(),
        }
    }

    // ── Theme ──

    pub fn current_mode(&self) -> ThemeMode {
        self.theme_handle.mode()
    }

    pub fn theme_config_path(&self) -> String {
        self.theme_handle.config_path()
    }

    pub fn system_dark(&self) -> bool {
        self.theme_handle.system_dark()
    }

    pub fn set_theme_mode(&mut self, mode: ThemeMode) {
        let label = mode.label();
        match self.theme_handle.set_mode(mode) {
            Ok(()) => self.message = format!("已切换为{label}"),
            Err(error) => self.message = format!("主题切换失败: {error}"),
        }
    }

    // ── Retention ──

    pub fn retention_seconds(&self) -> u64 {
        self.retention_draft
    }

    pub fn retention_status(&self) -> String {
        retention_status_text(self.retention_draft)
    }

    pub fn retention_message_text(&self) -> &str {
        &self.retention_message
    }

    pub fn set_retention_draft(&mut self, seconds: u64) {
        self.retention_draft = seconds.clamp(1, 3600);
        self.retention_message.clear();
    }

    pub fn adjust_retention(&mut self, delta: i64) {
        let new_value = if delta >= 0 {
            (self.retention_draft as i64)
                .saturating_add(delta)
                .min(3600) as u64
        } else {
            (self.retention_draft as i64).saturating_add(delta).max(1) as u64
        };
        self.retention_draft = new_value;
        self.retention_message.clear();
    }

    pub fn save_retention(&mut self) {
        match self.settings_store.lock() {
            Ok(mut store) => {
                match store.set_plugin_window_retention_seconds(self.retention_draft) {
                    Ok(saved) => {
                        self.retention_draft = saved;
                        self.retention_message = String::from("已保存");
                    }
                    Err(error) => {
                        self.retention_message = format!("保存失败: {error}");
                    }
                }
            }
            Err(_) => {
                self.retention_message = String::from("设置存储不可用");
            }
        }
    }

    pub fn restore_default_retention(&mut self) {
        match self.settings_store.lock() {
            Ok(mut store) => match store.restore_default_retention() {
                Ok(value) => {
                    self.retention_draft = value;
                    self.retention_message = String::from("已恢复默认");
                }
                Err(error) => {
                    self.retention_message = format!("恢复失败: {error}");
                }
            },
            Err(_) => {
                self.retention_message = String::from("设置存储不可用");
            }
        }
    }

    // ── App Index ──

    pub fn app_index_available(&self) -> bool {
        self.app_index_handle.is_some()
    }

    pub fn app_index_snapshot(&self) -> Option<AppIndexSnapshot> {
        self.app_index_handle.as_ref().map(|svc| svc.snapshot())
    }

    pub fn request_rescan(&mut self) -> bool {
        match &self.app_index_handle {
            Some(svc) => {
                if svc.request_scan() {
                    self.message = String::from("正在后台重新扫描应用");
                    true
                } else {
                    self.message = String::from("应用索引扫描已在进行中");
                    false
                }
            }
            None => {
                self.message = String::from("应用索引服务不可用");
                false
            }
        }
    }

    // ── Accessibility ──

    pub fn accessibility_status(&self) -> PermissionStatus {
        self.accessibility_status
    }

    pub fn accessibility_status_text(&self) -> String {
        let label = match self.accessibility_status {
            PermissionStatus::Authorized => "辅助功能权限：已授权",
            PermissionStatus::NotAuthorized => "辅助功能权限：未授权",
            PermissionStatus::Unknown => "辅助功能权限：未知（非 macOS 平台）",
        };
        label.to_string()
    }

    pub fn refresh_accessibility(&mut self) {
        self.accessibility_status = qingqi_platform::macos::check_accessibility();
    }

    pub fn open_accessibility_settings(&mut self) -> bool {
        let ok = qingqi_platform::macos::open_accessibility_settings();
        if ok {
            self.message = String::from("已打开系统设置");
        } else {
            self.message = String::from("打开系统设置失败");
        }
        // Re-read status after opening settings (user may have just toggled it)
        self.refresh_accessibility();
        ok
    }

    // ── Diagnostics paths ──

    pub fn data_dir_path(&self) -> String {
        self.app_paths.data_dir().display().to_string()
    }

    pub fn config_dir_path(&self) -> String {
        self.app_paths
            .data_dir()
            .join("config")
            .display()
            .to_string()
    }

    pub fn log_dir_path(&self) -> String {
        self.app_paths.data_dir().join("logs").display().to_string()
    }

    // ── Open directories ──

    pub fn open_data_dir(&mut self) {
        self.open_dir_action(&self.data_dir_path(), "数据目录");
    }

    pub fn open_config_dir(&mut self) {
        let path = self.config_dir_path();
        self.open_dir_action(&path, "配置目录");
    }

    pub fn open_log_dir(&mut self) {
        let path = self.log_dir_path();
        self.open_dir_action(&path, "日志目录");
    }

    fn open_dir_action(&mut self, path_str: &str, label: &str) {
        let path = std::path::Path::new(path_str);
        match qingqi_platform::shell::open_directory(path) {
            Ok(()) => {
                self.message = format!("已打开{label}");
            }
            Err(error) => {
                self.message = format!("打开{label}失败: {error}");
            }
        }
    }

    // ── Plugin directory ──

    pub fn imported_plugin_root_path(&self) -> String {
        self.app_paths.imported_plugins_dir().display().to_string()
    }

    pub fn open_plugin_dir(&mut self) {
        let path = self.app_paths.imported_plugins_dir();
        match qingqi_platform::shell::open_directory(&path) {
            Ok(()) => {
                self.message = format!("已打开插件目录: {}", path.display());
            }
            Err(error) => {
                self.message = format!("打开插件目录失败: {error}");
            }
        }
    }

    // ── Icon cache ──

    pub fn icon_cache_dir_path(&self) -> String {
        qingqi_platform::apps::icon_cache_dir()
            .display()
            .to_string()
    }

    pub fn icon_cache_message_text(&self) -> &str {
        &self.icon_cache_message
    }

    pub fn clear_icon_cache(&mut self) {
        match qingqi_platform::apps::clear_icon_cache_dir() {
            Ok(count) => {
                if count > 0 {
                    self.icon_cache_message =
                        format!("已清理 {count} 个缓存图标，下次重扫描时重建");
                } else {
                    self.icon_cache_message = String::from("图标缓存目录为空，无需清理");
                }
            }
            Err(error) => {
                self.icon_cache_message = format!("清理失败: {error}");
            }
        }
    }

    // ── Shortcuts ──

    pub fn shortcut_message_text(&self) -> &str {
        &self.shortcut_message
    }

    pub fn shortcut_rows(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Vec<(ShortcutView, Entity<TextInput>)> {
        let views = self
            .shortcut_handle
            .as_ref()
            .map(|service| service.views())
            .unwrap_or_default();

        for view in &views {
            let id = view.descriptor.id.clone();
            let value = view
                .normalized_accelerator
                .clone()
                .unwrap_or_else(|| view.descriptor.current_accelerator.clone());
            let editable = view.descriptor.editable;
            self.shortcut_drafts
                .entry(id.clone())
                .or_insert_with(|| value.clone());
            let input = self.shortcut_inputs.entry(id).or_insert_with(|| {
                cx.new(|cx| {
                    let mut input = TextInput::new(cx, "例如 Alt+V", value);
                    input.set_style(
                        TextInputStyle {
                            height: 30.0,
                            font_size: 12.0,
                            padding: 8.0,
                        },
                        cx,
                    );
                    input
                })
            });
            input.update(cx, |input, input_cx| {
                input.set_read_only(!editable, input_cx);
            });
        }

        views
            .into_iter()
            .filter_map(|view| {
                self.shortcut_inputs
                    .get(&view.descriptor.id)
                    .cloned()
                    .map(|input| (view, input))
            })
            .collect()
    }

    pub fn save_shortcut(
        &mut self,
        shortcut_id: &str,
        input: Entity<TextInput>,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        let accelerator = input.read(cx).text();
        let result = self
            .shortcut_handle
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("快捷键服务不可用"))
            .and_then(|service| service.set_shortcut(shortcut_id, &accelerator, enabled, cx));
        match result {
            Ok(()) => {
                self.shortcut_drafts
                    .insert(shortcut_id.to_string(), accelerator.clone());
                self.shortcut_message = if enabled {
                    format!("已保存快捷键 {accelerator}")
                } else {
                    String::from("已禁用快捷键")
                };
            }
            Err(error) => {
                self.shortcut_message = format!("快捷键保存失败: {error}");
            }
        }
    }

    pub fn restore_shortcut(&mut self, shortcut_id: &str, cx: &mut Context<Self>) {
        let result = self
            .shortcut_handle
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("快捷键服务不可用"))
            .and_then(|service| service.restore_shortcut(shortcut_id, cx));
        match result {
            Ok(()) => {
                self.shortcut_message = String::from("已恢复默认快捷键");
                if let Some(view) = self.shortcut_handle.as_ref().and_then(|service| {
                    service
                        .views()
                        .into_iter()
                        .find(|view| view.descriptor.id == shortcut_id)
                }) {
                    let value = view
                        .normalized_accelerator
                        .unwrap_or(view.descriptor.current_accelerator);
                    self.shortcut_drafts
                        .insert(shortcut_id.to_string(), value.clone());
                    if let Some(input) = self.shortcut_inputs.get(shortcut_id) {
                        input.update(cx, |input, input_cx| input.set_text(value, input_cx));
                    }
                }
            }
            Err(error) => {
                self.shortcut_message = format!("恢复默认失败: {error}");
            }
        }
    }
}

// ── Render ──

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let dark = qingqi_ui::theme_mode::is_dark();
        let message = self.message.clone();
        let current_mode = self.current_mode();
        let system_dark = self.system_dark();
        let config_path = self.theme_config_path();

        let retention_seconds = self.retention_seconds();
        let retention_status = self.retention_status();
        let retention_message = self.retention_message_text().to_string();

        let app_index_available = self.app_index_available();
        let app_snapshot: Option<AppIndexSnapshot> = self.app_index_snapshot();

        let data_dir = self.data_dir_path();
        let config_dir = self.config_dir_path();
        let log_dir = self.log_dir_path();

        let accessibility_status = self.accessibility_status();
        let accessibility_text = self.accessibility_status_text();

        let imported_plugin_root = self.imported_plugin_root_path();

        let icon_cache_dir = self.icon_cache_dir_path();
        let icon_cache_message = self.icon_cache_message_text().to_string();

        let has_app_snapshot = app_index_available && app_snapshot.is_some();

        let (shortcut_rows, shortcut_message) = {
            let rows = self.shortcut_rows(cx);
            let message = self.shortcut_message_text().to_string();
            (rows, message)
        };

        let header_message = if message.is_empty() {
            String::from("主题、窗口保留、应用索引与诊断信息")
        } else {
            message
        };

        let page_bg = theme::semantic().bg_page;
        let text_primary = theme::semantic().text_primary;
        let text_secondary = theme::semantic().text_secondary;

        div()
            .size_full()
            .bg(page_bg)
            .font_family(ui::font_ui())
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
            .child(components::settings_card(
                dark,
                "主题与外观",
                Some("控制台视觉样式"),
                div()
                    .flex()
                    .flex_col()
                    .child(components::settings_row(
                        dark,
                        "主题模式",
                        "切换浅色 / 深色 / 跟随系统外观",
                        mode_segment(entity.clone(), current_mode, dark),
                    ))
                    .child(components::settings_row(
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
                            .bg(theme::semantic().bg_subtle)
                            .flex()
                            .items_center()
                            .text_size(theme::font_size_caption())
                            .text_color(text_secondary)
                            .child(if system_dark { "深色" } else { "浅色" }),
                    )),
            ))
            // ── Plugin Retention ──
            .child(components::settings_card(
                dark,
                "插件管理",
                Some("窗口保留与导入管理"),
                div()
                    .flex()
                    .flex_col()
                    .child(components::settings_row(
                        dark,
                        "插件窗口保留",
                        &retention_status,
                        retention_control(
                            entity.clone(),
                            retention_seconds,
                            retention_message,
                            dark,
                        ),
                    ))
                    .child(components::settings_row(
                        dark,
                        "导入插件",
                        "目录/ZIP 导入尚未实现；可打开目标目录查看",
                        plugin_dir_button(entity.clone(), dark, &imported_plugin_root),
                    ))
                    .child(components::settings_row(
                        dark,
                        "已安装插件管理",
                        "管理已安装插件的启用/卸载",
                        disabled_badge("尚未实现"),
                    )),
            ))
            // ── Shortcuts ──
            .child(components::settings_card(
                dark,
                "快捷键",
                Some("全局与应用内快捷键"),
                shortcuts_section(entity.clone(), shortcut_rows, shortcut_message, dark),
            ))
            // ── App Index ──
            .child(components::settings_card(
                dark,
                "应用索引",
                Some("软件快速启动的应用缓存"),
                div().flex().flex_col().child(app_index_row(
                    entity.clone(),
                    dark,
                    has_app_snapshot,
                    app_snapshot,
                )),
            ))
            // ── macOS Permissions ──
            .child(components::settings_card(
                dark,
                "macOS 权限",
                Some("系统级访问授权状态"),
                div()
                    .flex()
                    .flex_col()
                    .child(accessibility_row(
                        entity.clone(),
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
            .child(components::settings_card(
                dark,
                "开发诊断",
                Some("数据、缓存与日志路径"),
                div()
                    .flex()
                    .flex_col()
                    .child(diag_path_row(
                        entity.clone(),
                        dark,
                        "数据目录",
                        "Qingqi 应用数据根目录",
                        &data_dir,
                        DiagAction::DataDir,
                    ))
                    .child(diag_path_row(
                        entity.clone(),
                        dark,
                        "配置目录",
                        "配置文件与数据库路径",
                        &config_dir,
                        DiagAction::ConfigDir,
                    ))
                    .child(diag_path_row(
                        entity.clone(),
                        dark,
                        "日志目录",
                        "运行日志输出目录",
                        &log_dir,
                        DiagAction::LogDir,
                    ))
                    .child(components::settings_row(
                        dark,
                        "主题配置",
                        "当前主题持久化文件",
                        path_badge(&config_path),
                    ))
                    .child(components::settings_row(
                        dark,
                        "应用索引维护",
                        "手动重建软件快速启动的应用索引",
                        app_index_action_button(entity.clone(), dark, has_app_snapshot),
                    ))
                    .child(components::settings_row(
                        dark,
                        "清理图标缓存",
                        &icon_cache_dir,
                        icon_cache_clear_button(entity.clone(), dark, icon_cache_message),
                    ))
                    .child(components::settings_row(
                        dark,
                        "日志诊断",
                        "后台服务状态、最近错误、警告统计",
                        disabled_badge("尚未实现"),
                    )),
            ))
    }
}

// ── Retention control ──

fn retention_control(
    entity: Entity<SettingsView>,
    seconds: u64,
    message: String,
    _dark: bool,
) -> impl IntoElement {
    let text_primary = theme::semantic().text_primary;
    let text_secondary = theme::semantic().text_secondary;

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
                .border_color(theme::semantic().border_default)
                .bg(theme::semantic().bg_surface)
                .hover(|style| style.bg(theme::semantic().bg_subtle).cursor_pointer())
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_body())
                .text_color(text_primary)
                .child("−")
                .on_click({
                    let entity = entity.clone();
                    move |_, _window, cx| {
                        entity.update(cx, |this, cx| {
                            this.adjust_retention(-30);
                            cx.notify();
                        });
                    }
                }),
        )
        .child(
            // Value display
            div()
                .h(px(28.0))
                .min_w(px(56.0))
                .rounded(theme::radius_sm())
                .bg(theme::semantic().bg_subtle)
                .border_1()
                .border_color(theme::semantic().border_default)
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
                .border_color(theme::semantic().border_default)
                .bg(theme::semantic().bg_surface)
                .hover(|style| style.bg(theme::semantic().bg_subtle).cursor_pointer())
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_body())
                .text_color(text_primary)
                .child("+")
                .on_click({
                    let entity = entity.clone();
                    move |_, _window, cx| {
                        entity.update(cx, |this, cx| {
                            this.adjust_retention(30);
                            cx.notify();
                        });
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
                .bg(theme::semantic().primary)
                .hover(|style| style.bg(theme::semantic().primary_hover).cursor_pointer())
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_caption())
                .text_color(theme::white())
                .child("保存")
                .on_click({
                    let entity = entity.clone();
                    move |_, _window, cx| {
                        entity.update(cx, |this, cx| {
                            this.save_retention();
                            cx.notify();
                        });
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
                .border_color(theme::semantic().border_default)
                .bg(theme::semantic().bg_surface)
                .hover(|style| style.bg(theme::semantic().bg_subtle).cursor_pointer())
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_caption())
                .text_color(text_primary)
                .child("默认")
                .on_click({
                    let entity = entity.clone();
                    move |_, _window, cx| {
                        entity.update(cx, |this, cx| {
                            this.restore_default_retention();
                            cx.notify();
                        });
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
    entity: Entity<SettingsView>,
    dark: bool,
    has_snapshot: bool,
    snapshot: Option<AppIndexSnapshot>,
) -> impl IntoElement {
    let text_secondary = theme::semantic().text_secondary;
    let text_primary = theme::semantic().text_primary;

    let (status_line, show_rescan) = if !has_snapshot {
        (String::from("应用索引服务不可用"), false)
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
            let entity = entity.clone();
            move |_, _window, cx| {
                entity.update(cx, |this, cx| {
                    this.request_rescan();
                    cx.notify();
                });
            }
        })
        .into_any_element()
    } else {
        disabled_badge("不可用").into_any_element()
    };

    div()
        .min_h(px(52.0))
        .px(theme::space_4())
        .py(theme::space_2())
        .border_b_1()
        .border_color(theme::semantic().border_default)
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
    entity: Entity<SettingsView>,
    dark: bool,
    available: bool,
) -> AnyElement {
    if available {
        action_button(dark, "重建索引", true, {
            let entity = entity.clone();
            move |_, _window, cx| {
                entity.update(cx, |this, cx| {
                    this.request_rescan();
                    cx.notify();
                });
            }
        })
        .into_any_element()
    } else {
        disabled_badge("服务不可用").into_any_element()
    }
}

fn plugin_dir_button(
    entity: Entity<SettingsView>,
    _dark: bool,
    _root_path: &str,
) -> impl IntoElement {
    div()
        .id("system-settings-open-plugin-dir")
        .h(px(28.0))
        .px_3()
        .rounded(theme::radius_md())
        .border_1()
        .border_color(theme::semantic().border_default)
        .bg(theme::semantic().bg_surface)
        .hover(|style| style.bg(theme::semantic().bg_subtle).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::font_size_caption())
        .text_color(theme::semantic().text_primary)
        .child("打开目录")
        .on_click(move |_, _window, cx| {
            entity.update(cx, |this, cx| {
                this.open_plugin_dir();
                cx.notify();
            });
        })
}

fn icon_cache_clear_button(
    entity: Entity<SettingsView>,
    _dark: bool,
    message: String,
) -> impl IntoElement {
    let text_secondary = theme::semantic().text_secondary;

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
                .bg(theme::semantic().primary)
                .hover(|style| style.bg(theme::semantic().primary_hover).cursor_pointer())
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_caption())
                .text_color(theme::white())
                .child("清理缓存")
                .on_click(move |_, _window, cx| {
                    entity.update(cx, |this, cx| {
                        this.clear_icon_cache();
                        cx.notify();
                    });
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
    entity: Entity<SettingsView>,
    rows: Vec<(ShortcutView, Entity<TextInput>)>,
    message: String,
    dark: bool,
) -> impl IntoElement {
    let text_secondary = theme::semantic().text_secondary;

    div()
        .flex()
        .flex_col()
        .when(!message.is_empty(), |el| {
            el.child(
                div()
                    .px(theme::space_4())
                    .py(theme::space_2())
                    .border_b_1()
                    .border_color(theme::semantic().border_default)
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
                .map(|(view, input)| shortcut_row(entity.clone(), view, input, dark)),
        )
}

fn shortcut_row(
    entity: Entity<SettingsView>,
    view: ShortcutView,
    input: Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    let text_primary = theme::semantic().text_primary;
    let text_secondary = theme::semantic().text_secondary;
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
    let status_style = shortcut_status_style(&view);
    let shortcut_id = descriptor.id.clone();
    let save_enabled = editable;

    div()
        .min_h(px(68.0))
        .px(theme::space_4())
        .py(theme::space_2())
        .border_b_1()
        .border_color(theme::semantic().border_default)
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
                        .child(scope_badge(scope_label, descriptor.scope))
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
                .child(shortcut_input_shell(input.clone(), editable))
                .child(shortcut_action_button(dark, "保存", true, save_enabled, {
                    let entity = entity.clone();
                    let shortcut_id = shortcut_id.clone();
                    let input = input.clone();
                    move |_, _window, cx| {
                        entity.update(cx, |this, cx| {
                            this.save_shortcut(&shortcut_id, input.clone(), true, cx);
                            cx.notify();
                        });
                    }
                }))
                .child(shortcut_action_button(
                    dark,
                    if enabled { "禁用" } else { "启用" },
                    false,
                    editable,
                    {
                        let entity = entity.clone();
                        let shortcut_id = shortcut_id.clone();
                        let input = input.clone();
                        move |_, _window, cx| {
                            entity.update(cx, |this, cx| {
                                this.save_shortcut(&shortcut_id, input.clone(), !enabled, cx);
                                cx.notify();
                            });
                        }
                    },
                ))
                .child(shortcut_action_button(dark, "默认", false, editable, {
                    let entity = entity.clone();
                    let shortcut_id = shortcut_id.clone();
                    move |_, _window, cx| {
                        entity.update(cx, |this, cx| {
                            this.restore_shortcut(&shortcut_id, cx);
                            cx.notify();
                        });
                    }
                })),
        )
}

fn shortcut_input_shell(input: Entity<TextInput>, editable: bool) -> impl IntoElement {
    div()
        .w(px(160.0))
        .rounded(theme::radius_sm())
        .border_1()
        .border_color(theme::semantic().border_default)
        .bg(if editable {
            theme::semantic().bg_surface
        } else {
            theme::semantic().bg_subtle
        })
        .child(input.into_any_element())
}

fn shortcut_action_button(
    _dark: bool,
    label: &'static str,
    primary: bool,
    enabled: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let bg = if !enabled {
        theme::semantic().bg_subtle
    } else if primary {
        theme::semantic().primary
    } else {
        theme::semantic().bg_surface
    };
    let text = if !enabled {
        theme::semantic().text_secondary
    } else if primary {
        theme::white()
    } else {
        theme::semantic().text_primary
    };

    div()
        .id(label)
        .h(px(28.0))
        .px_3()
        .rounded(theme::radius_md())
        .bg(bg)
        .border_1()
        .border_color(theme::semantic().border_default)
        .hover(move |style| {
            if enabled {
                style
                    .bg(if primary {
                        theme::semantic().primary_hover
                    } else {
                        theme::semantic().bg_subtle
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

fn shortcut_status_style(view: &ShortcutView) -> (gpui::Rgba, gpui::Rgba) {
    if view.error.is_some() || view.overridden_by.is_some() {
        return (
            theme::semantic().warning,
            theme::rgba_with_alpha(theme::semantic().warning, 0.1).into(),
        );
    }
    if !view.descriptor.enabled {
        return (
            theme::semantic().text_secondary,
            theme::rgba_with_alpha(theme::semantic().text_secondary, 0.08).into(),
        );
    }
    if view.active {
        return (
            theme::semantic().success,
            theme::rgba_with_alpha(theme::semantic().success, 0.1).into(),
        );
    }
    (
        theme::semantic().text_secondary,
        theme::rgba_with_alpha(theme::semantic().text_secondary, 0.08).into(),
    )
}

fn scope_badge(text: &'static str, scope: ShortcutScope) -> impl IntoElement {
    let color = match scope {
        ShortcutScope::Global => ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Slate),
        ShortcutScope::App => theme::semantic().text_secondary,
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

fn permission_row(
    _dark: bool,
    label: &'static str,
    description: &'static str,
    status: PermissionStatus,
) -> impl IntoElement {
    let (status_text, status_color, status_bg) = match status {
        PermissionStatus::Authorized => (
            "已授权",
            theme::semantic().success,
            theme::rgba_with_alpha(theme::semantic().success, 0.1),
        ),
        PermissionStatus::NotAuthorized => (
            "未授权",
            theme::semantic().warning,
            theme::rgba_with_alpha(theme::semantic().warning, 0.1),
        ),
        PermissionStatus::Unknown => (
            "尚未实现",
            theme::semantic().text_secondary,
            theme::rgba_with_alpha(theme::semantic().text_secondary, 0.08),
        ),
    };

    div()
        .min_h(px(52.0))
        .px(theme::space_4())
        .py(theme::space_2())
        .border_b_1()
        .border_color(theme::semantic().border_default)
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
                        .text_color(theme::semantic().text_primary)
                        .child(label),
                )
                .child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(theme::semantic().text_secondary)
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
    entity: Entity<SettingsView>,
    _dark: bool,
    status: PermissionStatus,
    text: &str,
) -> impl IntoElement {
    let text_secondary = theme::semantic().text_secondary;
    let text_primary = theme::semantic().text_primary;

    let (status_color, status_bg) = match status {
        PermissionStatus::Authorized => (
            theme::semantic().success,
            theme::rgba_with_alpha(theme::semantic().success, 0.1),
        ),
        PermissionStatus::NotAuthorized => (
            theme::semantic().warning,
            theme::rgba_with_alpha(theme::semantic().warning, 0.1),
        ),
        PermissionStatus::Unknown => (
            theme::semantic().text_secondary,
            theme::rgba_with_alpha(theme::semantic().text_secondary, 0.08),
        ),
    };

    div()
        .min_h(px(52.0))
        .px(theme::space_4())
        .py(theme::space_2())
        .border_b_1()
        .border_color(theme::semantic().border_default)
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
                        .bg(theme::semantic().primary)
                        .hover(|style| style.bg(theme::semantic().primary_hover).cursor_pointer())
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(theme::font_size_caption())
                        .text_color(theme::white())
                        .child("打开设置")
                        .on_click({
                            let entity = entity.clone();
                            move |_, _window, cx| {
                                entity.update(cx, |this, cx| {
                                    this.open_accessibility_settings();
                                    cx.notify();
                                });
                            }
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
    entity: Entity<SettingsView>,
    _dark: bool,
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
        .border_color(theme::semantic().border_default)
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
                        .text_color(theme::semantic().text_primary)
                        .child(label),
                )
                .child(path_badge(path)),
        )
        .child(
            div()
                .id(id_key)
                .h(px(28.0))
                .px_3()
                .rounded(theme::radius_md())
                .border_1()
                .border_color(theme::semantic().border_default)
                .bg(theme::semantic().bg_surface)
                .hover(|style| style.bg(theme::semantic().bg_subtle).cursor_pointer())
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::font_size_caption())
                .text_color(theme::semantic().text_primary)
                .child("打开")
                .on_click({
                    let entity = entity.clone();
                    move |_, _window, cx| {
                        entity.update(cx, |this, cx| {
                            match action {
                                DiagAction::DataDir => this.open_data_dir(),
                                DiagAction::ConfigDir => this.open_config_dir(),
                                DiagAction::LogDir => this.open_log_dir(),
                            }
                            cx.notify();
                        });
                    }
                }),
        )
}

fn disabled_badge(text: &'static str) -> impl IntoElement {
    let status_color = theme::semantic().text_secondary;
    let status_bg = theme::rgba_with_alpha(theme::semantic().text_secondary, 0.08);

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

fn path_badge(path: &str) -> impl IntoElement {
    div()
        .h(px(28.0))
        .px_2()
        .rounded(theme::radius_sm())
        .bg(theme::semantic().bg_subtle)
        .border_1()
        .border_color(theme::semantic().border_default)
        .flex()
        .items_center()
        .font_family("SF Mono")
        .text_size(theme::font_size_caption())
        .text_color(theme::semantic().text_secondary)
        .child(path.to_string())
}

fn action_button(
    _dark: bool,
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
            .bg(theme::semantic().primary)
            .hover(|style| style.bg(theme::semantic().primary_hover).cursor_pointer())
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
            .bg(theme::semantic().bg_surface)
            .border_1()
            .border_color(theme::semantic().border_default)
            .hover(|style| style.bg(theme::semantic().bg_subtle).cursor_pointer())
            .flex()
            .items_center()
            .justify_center()
            .text_size(theme::font_size_caption())
            .text_color(theme::semantic().text_primary)
            .child(label)
            .on_click(move |event, window, cx| on_click(event, window, cx))
    }
}

// ── Segmented Control for Theme Mode ──

fn mode_segment(
    entity: Entity<SettingsView>,
    current_mode: ThemeMode,
    dark: bool,
) -> impl IntoElement {
    div()
        .flex()
        .gap(px(2.0))
        .p(px(2.0))
        .rounded(theme::radius_md())
        .border_1()
        .border_color(theme::semantic().border_default)
        .bg(theme::semantic().bg_subtle)
        .child(seg_button(
            entity.clone(),
            ThemeMode::Light,
            current_mode,
            dark,
        ))
        .child(seg_button(
            entity.clone(),
            ThemeMode::Dark,
            current_mode,
            dark,
        ))
        .child(seg_button(
            entity.clone(),
            ThemeMode::System,
            current_mode,
            dark,
        ))
}

fn seg_button(
    entity: Entity<SettingsView>,
    mode: ThemeMode,
    current_mode: ThemeMode,
    _dark: bool,
) -> impl IntoElement {
    let active = current_mode == mode;
    let text_color = if active {
        theme::semantic().primary
    } else {
        theme::semantic().text_secondary
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
        .hover(move |style| style.bg(theme::semantic().bg_surface).cursor_pointer())
        .on_click({
            let entity = entity.clone();
            move |_, _window, cx| {
                entity.update(cx, |this, cx| {
                    this.set_theme_mode(mode);
                    cx.notify();
                });
            }
        });

    if active {
        btn = btn
            .bg(theme::semantic().bg_surface)
            .border_1()
            .border_color(theme::semantic().primary_soft)
            .shadow(vec![gpui::BoxShadow {
                color: theme::rgba_with_alpha(theme::semantic().shadow, 0.06),
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

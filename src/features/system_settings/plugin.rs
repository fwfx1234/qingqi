use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
};

use gpui::{AnyElement, App, AppContext, BorrowAppContext, Entity, IntoElement, Window};

use crate::{
    app::{
        app_index::{AppIndexService, AppIndexSnapshot},
        text_input::{TextInput, TextInputStyle},
        theme_store::{ThemeMode, ThemeStore},
    },
    core::{
        icon::IconRef,
        plugin::{InlineView, Plugin, PluginCx, PluginManifest, PluginView},
        plugin_spec::{
            PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
            PluginWindowMode, WindowSpec,
        },
        shortcut::{ShortcutService, ShortcutView},
        storage::AppPaths,
    },
    platform::macos::PermissionStatus,
};

use super::settings_store::{SettingsStore, retention_status_text};
use super::view::SettingsElement;

pub struct SystemSettingsRuntime {
    theme_store: Arc<Mutex<ThemeStore>>,
    settings_store: Arc<Mutex<SettingsStore>>,
    app_index_service: Option<Arc<AppIndexService>>,
    app_paths: AppPaths,
}

impl SystemSettingsRuntime {
    pub fn new(
        theme_store: Arc<Mutex<ThemeStore>>,
        app_paths: AppPaths,
        settings_store: Arc<Mutex<SettingsStore>>,
        app_index_service: Option<Arc<AppIndexService>>,
    ) -> Self {
        Self {
            theme_store,
            settings_store,
            app_index_service,
            app_paths,
        }
    }

    pub fn manifest_static() -> PluginManifest {
        PluginManifest {
            id: "system-settings".into(),
            name: "系统设置".into(),
            description: "主题切换与应用偏好设置".into(),
            keywords: ["设置", "settings", "主题", "theme", "偏好"]
                .into_iter()
                .map(Into::into)
                .collect(),
            background: false,
            dynamic_commands: false,
            visual: PluginVisualSpec {
                icon: IconRef::asset("qta/mdi6.cog-outline.png"),
                accent: PluginAccent::Slate,
                category: PluginCategory::System,
                status: PluginStatus::Ready,
                mode: PluginWindowMode::Inline,
                window: WindowSpec::ratio(0.72, 0.7),
            },
            stats: PluginStats {
                primary: "主题设置".into(),
                secondary: "配置持久化".into(),
                tertiary: "偏好设置".into(),
            },
            command_hint: "主题、窗口保留、应用索引与诊断信息".into(),
            command_prefixes: ["set", "settings"].into_iter().map(Into::into).collect(),
        }
    }
}

impl Plugin for SystemSettingsRuntime {
    fn manifest(&self) -> PluginManifest {
        Self::manifest_static()
    }

    fn open(&mut self, _: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let panel = SettingsPanel::new(
            Arc::clone(&self.theme_store),
            Arc::clone(&self.settings_store),
            self.app_index_service.clone(),
            self.app_paths.clone(),
        );
        Ok(PluginView::Inline(Box::new(SystemSettingsView {
            panel: Rc::new(RefCell::new(panel)),
        })))
    }

    fn close_idle(&mut self) {}
}

pub struct SystemSettingsView {
    panel: Rc<RefCell<SettingsPanel>>,
}

impl InlineView for SystemSettingsView {
    fn plugin_id(&self) -> &str {
        "system-settings"
    }

    fn title(&self) -> &str {
        "系统设置"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        SettingsElement {
            panel: Rc::clone(&self.panel),
        }
        .into_any_element()
    }

    fn on_close(&mut self) {
        self.panel.borrow_mut().message.clear();
    }
}

pub struct SettingsPanel {
    theme_store: Arc<Mutex<ThemeStore>>,
    settings_store: Arc<Mutex<SettingsStore>>,
    app_index_service: Option<Arc<AppIndexService>>,
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

impl SettingsPanel {
    fn new(
        theme_store: Arc<Mutex<ThemeStore>>,
        settings_store: Arc<Mutex<SettingsStore>>,
        app_index_service: Option<Arc<AppIndexService>>,
        app_paths: AppPaths,
    ) -> Self {
        let retention = settings_store
            .lock()
            .ok()
            .map(|store| store.plugin_window_retention_seconds())
            .unwrap_or(300);
        let accessibility_status = crate::platform::macos::check_accessibility();
        Self {
            theme_store,
            settings_store,
            app_index_service,
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
        self.theme_store
            .lock()
            .ok()
            .map(|store| store.mode())
            .unwrap_or(ThemeMode::System)
    }

    pub fn theme_config_path(&self) -> String {
        self.theme_store
            .lock()
            .ok()
            .map(|store| store.config_path().display().to_string())
            .unwrap_or_default()
    }

    pub fn system_dark(&self) -> bool {
        self.theme_store
            .lock()
            .ok()
            .map(|store| store.system_dark())
            .unwrap_or(false)
    }

    pub fn set_theme_mode(&mut self, mode: ThemeMode) {
        let label = mode.label();
        match self.theme_store.lock() {
            Ok(mut store) => match store.set_mode(mode) {
                Ok(()) => self.message = format!("已切换为{label}"),
                Err(error) => self.message = format!("主题切换失败: {error}"),
            },
            Err(_) => self.message = String::from("主题存储不可用"),
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
        self.app_index_service.is_some()
    }

    pub fn app_index_snapshot(&self) -> Option<AppIndexSnapshot> {
        self.app_index_service.as_ref().map(|svc| svc.snapshot())
    }

    pub fn request_rescan(&mut self) -> bool {
        match &self.app_index_service {
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
        self.accessibility_status = crate::platform::macos::check_accessibility();
    }

    pub fn open_accessibility_settings(&mut self) -> bool {
        let ok = crate::platform::macos::open_accessibility_settings();
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
        match crate::platform::shell::open_directory(path) {
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
        match crate::platform::shell::open_directory(&path) {
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
        crate::platform::apps::icon_cache_dir()
            .display()
            .to_string()
    }

    pub fn icon_cache_message_text(&self) -> &str {
        &self.icon_cache_message
    }

    pub fn clear_icon_cache(&mut self) {
        match crate::platform::apps::clear_icon_cache_dir() {
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

    pub fn shortcut_rows(&mut self, cx: &mut App) -> Vec<(ShortcutView, Entity<TextInput>)> {
        let views = cx
            .try_global::<ShortcutService>()
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
        cx: &mut App,
    ) {
        let accelerator = input.read(cx).text();
        let result = cx.update_global::<ShortcutService, _>(|service, cx| {
            service.set_shortcut(shortcut_id, &accelerator, enabled, cx)
        });
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

    pub fn restore_shortcut(&mut self, shortcut_id: &str, cx: &mut App) {
        let result = cx.update_global::<ShortcutService, _>(|service, cx| {
            service.restore_shortcut(shortcut_id, cx)
        });
        match result {
            Ok(()) => {
                self.shortcut_message = String::from("已恢复默认快捷键");
                if let Some(view) = cx.try_global::<ShortcutService>().and_then(|service| {
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

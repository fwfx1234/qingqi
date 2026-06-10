//! SshView 主视图 + ViewModel + 布局

mod app_settings;
mod context_menu;
mod file_context_menu;
mod file_tree;
mod profile_editor;
mod session_tabs;
mod sidebar;
mod terminal_pane;
mod transfer_panel;

use std::sync::Arc;

use crate::model::{AuthConfig, ProtocolType, SessionId, SessionStatus, SshAuthMethod, TerminalKind};
use crate::service::{SshEvent, SshService};
use crate::terminal::TerminalLine;
use crate::transfer;
use gpui::prelude::FluentBuilder;
use gpui::*;
use qingqi_ui::text_input::TextInput;

// ========== ViewModel (render-ready 纯数据) ==========

#[derive(Clone, Debug)]
pub struct ProfileItem {
    pub id: i64,
    pub name: String,
    pub endpoint: String,
    pub protocol_badge: String,
    pub is_connected: bool,
    pub is_selected: bool,
}

#[derive(Clone, Debug)]
pub struct SessionTabItem {
    pub session_id: SessionId,
    pub title: String,
    pub is_selected: bool,
    pub status_color: Hsla,
}

#[derive(Clone, Debug)]
pub struct FileTreeViewModel {
    pub current_path: String,
    pub parent_path: Option<String>,
    pub entries: Vec<FileEntryRow>,
}

#[derive(Clone, Debug)]
pub struct FileEntryRow {
    pub path: String,
    pub name: String,
    pub icon_name: String,
    pub size_text: String,
    pub modified_text: String,
    pub is_dir: bool,
    pub is_parent: bool,
    pub is_selected: bool,
}

#[derive(Clone, Debug)]
pub struct TerminalViewModel {
    pub status: String,
    pub lines: Vec<TerminalLine>,
    pub cursor_visible: bool,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub terminal_kind: TerminalKind,
}

#[derive(Clone, Debug)]
pub struct TransferPanelViewModel {
    pub active_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub rows: Vec<TransferRowViewModel>,
}

#[derive(Clone, Debug)]
pub struct TransferRowViewModel {
    pub id: crate::model::TransferId,
    pub direction_icon: &'static str,
    pub file_name: String,
    pub progress_percent: u8,
    pub status_text: String,
    pub status_color: Hsla,
    pub speed_text: String,
    pub logs: Vec<String>,
    pub expanded: bool,
}

#[derive(Clone, Debug)]
pub struct SshViewModel {
    pub profiles: Vec<ProfileItem>,
    pub sessions: Vec<SessionTabItem>,
    pub file_tree: FileTreeViewModel,
    pub terminal: TerminalViewModel,
    pub transfers: TransferPanelViewModel,
}

// ========== SshView ==========

pub struct SshView {
    service: Arc<SshService>,
    focus_handle: FocusHandle,

    vm: SshViewModel,

    selected_profile_id: Option<i64>,
    selected_session_id: Option<SessionId>,
    transfer_panel_expanded: bool,
    show_profile_editor: bool,
    show_app_settings: bool,
    editing_profile_id: Option<i64>, // None=新建, Some(id)=编辑
    form_advanced_expanded: bool,
    context_menu_profile_id: Option<i64>,
    context_menu_position: Option<Point<Pixels>>,
    file_context_menu_entry: Option<FileEntryRow>,
    file_context_menu_position: Option<Point<Pixels>>,
    terminal_font_size: f32,

    // Profile 表单状态
    form_protocol: ProtocolType,
    form_auth_method: SshAuthMethod,

    // Profile 表单输入框
    form_name: Entity<TextInput>,
    form_host: Entity<TextInput>,
    form_port: Entity<TextInput>,
    form_username: Entity<TextInput>,
    form_password: Entity<TextInput>,
    form_remote_root: Entity<TextInput>,
    form_local_root: Entity<TextInput>,
    form_private_key_path: Entity<TextInput>,
    form_private_key_passphrase: Entity<TextInput>,
    form_note: Entity<TextInput>,
    form_connection_timeout: Entity<TextInput>,
    form_keepalive_interval: Entity<TextInput>,
    form_terminal_font_size: Entity<TextInput>,

    event_task: Option<Task<()>>,
    generation: u64,
    /// 终端等高频更新节流
    last_vm_refresh: Option<std::time::Instant>,
}

fn plain_input(
    cx: &mut Context<TextInput>,
    placeholder: impl Into<SharedString>,
    value: impl Into<SharedString>,
) -> TextInput {
    let mut input = TextInput::new(cx, placeholder, value);
    input.set_chrome(false, cx);
    input
}

impl SshView {
    pub fn new(service: Arc<SshService>, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            service: Arc::clone(&service),
            focus_handle: cx.focus_handle(),
            vm: SshViewModel::default(),
            selected_profile_id: None,
            selected_session_id: None,
            transfer_panel_expanded: false,
            show_profile_editor: false,
            show_app_settings: false,
            editing_profile_id: None,
            form_advanced_expanded: false,
            context_menu_profile_id: None,
            context_menu_position: None,
            file_context_menu_entry: None,
            file_context_menu_position: None,
            terminal_font_size: 12.0,
            form_protocol: ProtocolType::Ssh,
            form_auth_method: SshAuthMethod::Password { password: String::new() },
            form_name: cx.new(|cx| plain_input(cx, "名称", "")),
            form_host: cx.new(|cx| plain_input(cx, "主机地址", "")),
            form_port: cx.new(|cx| plain_input(cx, "端口", "22")),
            form_username: cx.new(|cx| plain_input(cx, "用户名", "root")),
            form_password: cx.new(|cx| plain_input(cx, "密码", "")),
            form_remote_root: cx.new(|cx| plain_input(cx, "远程目录", "~")),
            form_local_root: cx.new(|cx| plain_input(cx, "本地目录", "~/Downloads")),
            form_private_key_path: cx.new(|cx| plain_input(cx, "私钥路径", "~/.ssh/id_rsa")),
            form_private_key_passphrase: cx.new(|cx| plain_input(cx, "私钥密码", "")),
            form_note: cx.new(|cx| plain_input(cx, "备注", "")),
            form_connection_timeout: cx.new(|cx| plain_input(cx, "秒", "30")),
            form_keepalive_interval: cx.new(|cx| plain_input(cx, "秒", "60")),
            form_terminal_font_size: cx.new(|cx| plain_input(cx, "字号", "12")),
            event_task: None,
            generation: 0,
            last_vm_refresh: None,
        };
        this.rebuild_view_model();
        this.start_event_loop(cx);
        this
    }

    fn start_event_loop(&mut self, cx: &mut Context<Self>) {
        let service = Arc::clone(&self.service);
        let mut rx = service.subscribe();

        self.generation = self.generation.wrapping_add(1);
        let task_gen = self.generation;

        self.event_task = Some(cx.spawn(async move |view, acx| {
            while let Ok(event) = rx.recv().await {
                let _ = view.update(acx, |view, cx| {
                    if view.generation != task_gen {
                        return;
                    }
                    view.on_service_event(&event, cx);
                });
            }
        }));
    }

    fn on_service_event(&mut self, event: &SshEvent, cx: &mut Context<Self>) {
        tracing::debug!(target: "qingqi_ssh", ?event, "view: service event");
        match event {
            // 终端输出等高频事件：节流 rebuild，避免每字节都全量重建 vm
            SshEvent::SessionDataChanged(_) => self.refresh_ui_throttled(cx),
            _ => self.refresh_ui(cx),
        }
    }

    /// 从 Service 拉取最新数据并通知 GPUI 重绘
    fn refresh_ui(&mut self, cx: &mut Context<Self>) {
        self.rebuild_view_model();
        tracing::debug!(
            target: "qingqi_ssh",
            profiles = self.vm.profiles.len(),
            sessions = self.vm.sessions.len(),
            terminal_lines = self.vm.terminal.lines.len(),
            file_entries = self.vm.file_tree.entries.len(),
            "view: refresh_ui"
        );
        cx.notify();
    }

    /// 高频刷新节流（默认 ~50ms 最多 rebuild 一次）
    pub(crate) fn terminal_input_enabled(&self) -> bool {
        !self.show_profile_editor && !self.show_app_settings
    }

    pub(crate) fn refresh_ui_throttled(&mut self, cx: &mut Context<Self>) {
        const MIN_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);
        let now = std::time::Instant::now();
        if self
            .last_vm_refresh
            .is_some_and(|t| now.duration_since(t) < MIN_INTERVAL)
        {
            return;
        }
        self.last_vm_refresh = Some(now);
        self.refresh_ui(cx);
    }

    // ===== 用户交互 =====

    /// 单击选中 Profile
    pub(crate) fn select_profile(&mut self, profile_id: i64, cx: &mut Context<Self>) {
        self.close_context_menu(cx);
        self.selected_profile_id = Some(profile_id);
        self.refresh_ui(cx);
    }

    /// 双击 / 右键菜单连接
    pub(crate) fn connect_profile(&mut self, profile_id: i64, cx: &mut Context<Self>) {
        self.close_context_menu(cx);
        self.selected_profile_id = Some(profile_id);
        tracing::debug!(
            target: "qingqi_ssh",
            profile_id,
            "用户发起连接"
        );
        match self.service.open_session(profile_id) {
            Ok(session_id) => {
                self.selected_session_id = Some(session_id);
            }
            Err(e) => {
                tracing::error!(target: "qingqi_ssh", profile_id, error = %e, "连接发起失败");
            }
        }
        self.refresh_ui(cx);
    }

    fn select_session(&mut self, session_id: SessionId, cx: &mut Context<Self>) {
        self.selected_session_id = Some(session_id);
        self.refresh_ui(cx);
    }

    /// 刷新当前远程目录列表
    pub(crate) fn refresh_file_tree(&mut self, cx: &mut Context<Self>) {
        let Some(sid) = self.selected_session_id else {
            return;
        };
        let path = self.service.session_cwd(&sid);
        if path.is_empty() {
            return;
        }
        match self.service.list_directory(&sid, &path) {
            Ok(_) => self.refresh_ui(cx),
            Err(e) => tracing::warn!(target: "qingqi_ssh", error = %e, "刷新目录失败"),
        }
    }

    /// 进入远程目录（双击目录或 `..`）
    pub(crate) fn navigate_to_remote_path(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(sid) = self.selected_session_id else {
            return;
        };
        tracing::debug!(target: "qingqi_ssh", path, "导航到远程目录");
        match self.service.list_directory(&sid, path) {
            Ok(_) => self.refresh_ui(cx),
            Err(e) => tracing::warn!(target: "qingqi_ssh", path, error = %e, "进入目录失败"),
        }
    }

    /// 双击文件列表项
    pub(crate) fn open_file_entry(&mut self, entry: &FileEntryRow, cx: &mut Context<Self>) {
        if entry.is_parent {
            self.navigate_to_remote_path(&entry.path, cx);
        } else if entry.is_dir {
            self.navigate_to_remote_path(&entry.path, cx);
        }
    }

    fn close_session(&mut self, session_id: SessionId, cx: &mut Context<Self>) {
        if self.selected_session_id == Some(session_id) {
            self.selected_session_id = None;
        }
        let _ = self.service.close_session(&session_id);
        self.refresh_ui(cx);
    }

    fn toggle_transfer_panel(&mut self, cx: &mut Context<Self>) {
        self.transfer_panel_expanded = !self.transfer_panel_expanded;
        cx.notify();
    }

    // ===== 右键菜单 =====

    fn open_context_menu(&mut self, profile_id: i64, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.close_file_context_menu(cx);
        self.context_menu_profile_id = Some(profile_id);
        self.context_menu_position = Some(position);
        cx.notify();
    }

    pub(crate) fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        self.context_menu_profile_id = None;
        self.context_menu_position = None;
        cx.notify();
    }

    pub(crate) fn open_file_context_menu(
        &mut self,
        entry: Option<FileEntryRow>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.close_context_menu(cx);
        self.file_context_menu_entry = entry;
        self.file_context_menu_position = Some(position);
        cx.notify();
    }

    pub(crate) fn close_file_context_menu(&mut self, cx: &mut Context<Self>) {
        self.file_context_menu_entry = None;
        self.file_context_menu_position = None;
        cx.notify();
    }

    pub(crate) fn create_directory_in_cwd(&mut self, cx: &mut Context<Self>) {
        let Some(sid) = self.selected_session_id else {
            return;
        };
        let cwd = self.service.session_cwd(&sid);
        if cwd.is_empty() {
            return;
        }
        let path = format!("{}/新建文件夹", cwd.trim_end_matches('/'));
        match self.service.create_remote_directory(&sid, &path) {
            Ok(()) => {
                let _ = self.service.list_directory(&sid, &cwd);
                self.refresh_ui(cx);
            }
            Err(e) => tracing::warn!(target: "qingqi_ssh", error = %e, "创建目录失败"),
        }
    }

    pub(crate) fn download_file_entry(&mut self, entry: &FileEntryRow, cx: &mut Context<Self>) {
        let Some(sid) = self.selected_session_id else {
            return;
        };
        let local_root = self.form_local_root.read(cx).text();
        let local = std::path::PathBuf::from(local_root).join(&entry.name);
        match self.service.download_file(&sid, &entry.path, &local) {
            Ok(_) => tracing::debug!(target: "qingqi_ssh", path = %entry.path, "开始下载"),
            Err(e) => tracing::warn!(target: "qingqi_ssh", error = %e, "下载失败"),
        }
    }

    // ===== Profile 编辑弹窗 =====

    pub(crate) fn open_profile_editor(&mut self, profile_id: Option<i64>, cx: &mut Context<Self>) {
        self.close_context_menu(cx);
        self.close_app_settings(cx);
        self.editing_profile_id = profile_id;
        self.show_profile_editor = true;
        self.form_advanced_expanded = false;

        if let Some(id) = profile_id {
            if let Ok(Some(profile)) = self.service.get_profile(id) {
                self.fill_form_from_profile(&profile, cx);
            }
        } else {
            self.reset_form(cx);
        }
        cx.notify();
    }

    pub(crate) fn close_profile_editor(&mut self, cx: &mut Context<Self>) {
        self.show_profile_editor = false;
        self.editing_profile_id = None;
        self.form_advanced_expanded = false;
        cx.notify();
    }

    pub(crate) fn toggle_form_advanced(&mut self, cx: &mut Context<Self>) {
        self.form_advanced_expanded = !self.form_advanced_expanded;
        cx.notify();
    }

    fn fill_form_from_profile(&mut self, profile: &crate::model::Profile, cx: &mut Context<Self>) {
        self.form_name.update(cx, |input, cx| input.set_text(&profile.name, cx));
        self.form_host.update(cx, |input, cx| input.set_text(&profile.host, cx));
        self.form_port.update(cx, |input, cx| {
            input.set_text(&profile.port.to_string(), cx)
        });
        self.form_remote_root.update(cx, |input, cx| {
            input.set_text(&profile.paths.remote_root, cx)
        });
        self.form_local_root.update(cx, |input, cx| {
            input.set_text(&profile.paths.local_root, cx)
        });
        self.form_note.update(cx, |input, cx| input.set_text(&profile.note, cx));
        self.form_connection_timeout.update(cx, |input, cx| {
            input.set_text(
                &profile.advanced.connection_timeout_secs.to_string(),
                cx,
            )
        });
        self.form_keepalive_interval.update(cx, |input, cx| {
            input.set_text(
                &profile.advanced.keepalive_interval_secs.to_string(),
                cx,
            )
        });
        self.form_protocol = profile.protocol.clone();
        self.form_auth_method = match &profile.auth {
            AuthConfig::Ssh { username, method } => {
                self.form_username.update(cx, |input, cx| input.set_text(username, cx));
                match method {
                    SshAuthMethod::Password { password } => {
                        self.form_password.update(cx, |input, cx| {
                            input.set_text(password, cx)
                        });
                    }
                    SshAuthMethod::PrivateKey { path, passphrase } => {
                        self.form_private_key_path.update(cx, |input, cx| {
                            input.set_text(path, cx)
                        });
                        self.form_private_key_passphrase.update(cx, |input, cx| {
                            input.set_text(passphrase, cx)
                        });
                    }
                    SshAuthMethod::Agent => {}
                }
                method.clone()
            }
            AuthConfig::Ftp { username, password } => {
                self.form_username.update(cx, |input, cx| input.set_text(username, cx));
                self.form_password.update(cx, |input, cx| input.set_text(password, cx));
                SshAuthMethod::Password {
                    password: password.clone(),
                }
            }
        };
    }

    fn reset_form(&mut self, cx: &mut Context<Self>) {
        self.form_protocol = ProtocolType::Ssh;
        self.form_auth_method = SshAuthMethod::Password { password: String::new() };
        self.form_name.update(cx, |input, cx| input.set_text("", cx));
        self.form_host.update(cx, |input, cx| input.set_text("", cx));
        self.form_port.update(cx, |input, cx| input.set_text("22", cx));
        self.form_username.update(cx, |input, cx| input.set_text("root", cx));
        self.form_password.update(cx, |input, cx| input.set_text("", cx));
        self.form_remote_root.update(cx, |input, cx| input.set_text("~", cx));
        self.form_local_root.update(cx, |input, cx| input.set_text("~/Downloads", cx));
        self.form_private_key_path.update(cx, |input, cx| {
            input.set_text("~/.ssh/id_rsa", cx)
        });
        self.form_private_key_passphrase.update(cx, |input, cx| input.set_text("", cx));
        self.form_note.update(cx, |input, cx| input.set_text("", cx));
        self.form_connection_timeout.update(cx, |input, cx| input.set_text("30", cx));
        self.form_keepalive_interval.update(cx, |input, cx| input.set_text("60", cx));
    }

    // ===== 插件设置弹窗 =====

    pub(crate) fn open_app_settings(&mut self, cx: &mut Context<Self>) {
        self.close_context_menu(cx);
        self.close_profile_editor(cx);
        self.show_app_settings = true;
        self.form_terminal_font_size.update(cx, |input, cx| {
            input.set_text(&self.terminal_font_size.round().to_string(), cx)
        });
        cx.notify();
    }

    pub(crate) fn close_app_settings(&mut self, cx: &mut Context<Self>) {
        self.show_app_settings = false;
        cx.notify();
    }

    pub(crate) fn save_app_settings(&mut self, cx: &mut Context<Self>) {
        if let Ok(size) = self
            .form_terminal_font_size
            .read(cx)
            .text()
            .parse::<f32>()
        {
            self.terminal_font_size = size.clamp(10.0, 20.0);
        }
        self.close_app_settings(cx);
        cx.notify();
    }

    fn set_form_protocol(&mut self, protocol: ProtocolType, cx: &mut Context<Self>) {
        self.form_protocol = protocol;
        cx.notify();
    }

    fn set_form_auth_method(&mut self, method: SshAuthMethod, cx: &mut Context<Self>) {
        self.form_auth_method = method;
        cx.notify();
    }

    pub(crate) fn save_profile_from_form(&mut self, cx: &mut Context<Self>) {
        let name = self.form_name.read(cx).text();
        let host = self.form_host.read(cx).text();
        let port: u16 = self.form_port.read(cx).text().parse().unwrap_or(22);
        let remote_root = self.form_remote_root.read(cx).text();
        let local_root = self.form_local_root.read(cx).text();
        let note = self.form_note.read(cx).text();
        let connection_timeout_secs = self
            .form_connection_timeout
            .read(cx)
            .text()
            .parse()
            .unwrap_or(30);
        let keepalive_interval_secs = self
            .form_keepalive_interval
            .read(cx)
            .text()
            .parse()
            .unwrap_or(60);

        if name.is_empty() || host.is_empty() {
            return;
        }

        let auth = match self.form_protocol {
            ProtocolType::Ftp | ProtocolType::Ftps => {
                let username = self.form_username.read(cx).text();
                let password = self.form_password.read(cx).text();
                AuthConfig::Ftp { username, password }
            }
            ProtocolType::Ssh => {
                let username = self.form_username.read(cx).text();
                let method = match &self.form_auth_method {
                    SshAuthMethod::Password { .. } => {
                        let password = self.form_password.read(cx).text();
                        SshAuthMethod::Password { password }
                    }
                    SshAuthMethod::PrivateKey { .. } => SshAuthMethod::PrivateKey {
                        path: self.form_private_key_path.read(cx).text(),
                        passphrase: self.form_private_key_passphrase.read(cx).text(),
                    },
                    SshAuthMethod::Agent => SshAuthMethod::Agent,
                };
                AuthConfig::Ssh { username, method }
            }
        };

        let draft = crate::model::ProfileDraft {
            name,
            protocol: self.form_protocol.clone(),
            host,
            port,
            auth,
            paths: crate::model::PathConfig {
                remote_root,
                local_root,
            },
            advanced: crate::model::ProfileAdvanced {
                connection_timeout_secs,
                keepalive_interval_secs,
            },
            note,
        };

        let result = if let Some(id) = self.editing_profile_id {
            self.service.update_profile(id, draft)
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("{e}"))
        } else {
            self.service.create_profile(draft).map(|_| ())
        };

        match result {
            Ok(()) => {
                self.close_profile_editor(cx);
                self.refresh_ui(cx);
            }
            Err(e) => {
                tracing::error!(target: "qingqi_ssh", error = %e, "保存 Profile 失败");
            }
        }
    }

    pub(crate) fn delete_profile(&mut self, profile_id: i64, cx: &mut Context<Self>) {
        self.close_context_menu(cx);
        match self.service.delete_profile(profile_id) {
            Ok(true) => {
                self.refresh_ui(cx);
            }
            Ok(false) => {
                tracing::warn!("Profile {profile_id} 不存在，无法删除");
            }
            Err(e) => {
                tracing::error!("删除 Profile 失败: {e}");
            }
        }
    }

    fn rebuild_view_model(&mut self) {
        let snap = self.service.snapshot();
        self.vm = SshViewModel {
            profiles: Self::build_profiles(
                &snap.profiles,
                &snap.sessions,
                self.selected_profile_id,
            ),
            sessions: Self::build_sessions(&snap.sessions, self.selected_session_id),
            file_tree: Self::build_file_tree(self.selected_session_id.as_ref(), &self.service),
            terminal: Self::build_terminal(self.selected_session_id.as_ref(), &self.service),
            transfers: Self::build_transfers(self.selected_session_id.as_ref(), &self.service),
        };
    }

    // ===== Build Functions =====

    fn build_profiles(
        profiles: &[crate::model::Profile],
        sessions: &[crate::model::SessionSummary],
        selected_id: Option<i64>,
    ) -> Vec<ProfileItem> {
        profiles
            .iter()
            .map(|p| {
                let is_connected = sessions
                    .iter()
                    .any(|s| s.profile_id == p.id && matches!(s.status, SessionStatus::Connected));
                ProfileItem {
                    id: p.id,
                    name: p.name.clone(),
                    endpoint: format!("{}:{}", p.host, p.port),
                    protocol_badge: p.protocol.display().to_string(),
                    is_connected,
                    is_selected: selected_id == Some(p.id),
                }
            })
            .collect()
    }

    fn build_sessions(
        sessions: &[crate::model::SessionSummary],
        selected_id: Option<SessionId>,
    ) -> Vec<SessionTabItem> {
        sessions
            .iter()
            .map(|s| SessionTabItem {
                session_id: s.session_id,
                title: s.title.clone(),
                is_selected: selected_id == Some(s.session_id),
                status_color: match s.status {
                    SessionStatus::Connecting => hsla(0.14, 0.8, 0.5, 1.0),
                    SessionStatus::Connected => hsla(0.4, 0.8, 0.5, 1.0),
                    SessionStatus::Failed => hsla(0.0, 0.8, 0.5, 1.0),
                },
            })
            .collect()
    }

    fn join_remote_path(base: &str, name: &str) -> String {
        let base = base.trim_end_matches('/');
        if base.is_empty() {
            format!("/{name}")
        } else {
            format!("{base}/{name}")
        }
    }

    fn build_file_tree(session_id: Option<&SessionId>, service: &SshService) -> FileTreeViewModel {
        let (current_path, entries) = session_id
            .map(|id| {
                let cwd = service.session_cwd(id);
                let ents = service.session_entries(id);
                (cwd, ents)
            })
            .unwrap_or_default();

        let parent = if current_path == "/" || current_path.is_empty() {
            None
        } else {
            let p = std::path::Path::new(&current_path);
            p.parent().map(|p| p.to_string_lossy().to_string())
        };

        let mut rows: Vec<FileEntryRow> = entries
            .into_iter()
            .filter(|e| e.name != "." && e.name != "..")
            .map(|e| {
                let path = if e.path.is_empty() {
                    Self::join_remote_path(&current_path, &e.name)
                } else {
                    e.path
                };
                FileEntryRow {
                    path,
                    name: e.name.clone(),
                    icon_name: String::new(),
                    size_text: if e.is_dir {
                        String::new()
                    } else {
                        transfer::format_size(e.size)
                    },
                    modified_text: transfer::format_modified(&e.modified_at),
                    is_dir: e.is_dir,
                    is_parent: false,
                    is_selected: false,
                }
            })
            .collect();
        rows.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        if let Some(parent) = &parent {
            rows.insert(
                0,
                FileEntryRow {
                    path: parent.clone(),
                    name: "..".into(),
                    icon_name: String::new(),
                    size_text: String::new(),
                    modified_text: String::new(),
                    is_dir: true,
                    is_parent: true,
                    is_selected: false,
                },
            );
        }

        FileTreeViewModel {
            current_path,
            parent_path: parent,
            entries: rows,
        }
    }

    fn build_terminal(session_id: Option<&SessionId>, service: &SshService) -> TerminalViewModel {
        session_id
            .and_then(|id| service.terminal_snapshot(id))
            .map(|frame| TerminalViewModel {
                status: frame.status_text,
                lines: frame.lines,
                cursor_visible: frame.cursor_visible,
                cursor_row: frame.cursor_row,
                cursor_col: frame.cursor_col,
                terminal_kind: frame.terminal_kind,
            })
            .unwrap_or(TerminalViewModel {
                status: "未连接".into(),
                lines: Vec::new(),
                cursor_visible: false,
                cursor_row: 0,
                cursor_col: 0,
                terminal_kind: TerminalKind::Shell,
            })
    }

    fn build_transfers(
        session_id: Option<&SessionId>,
        service: &SshService,
    ) -> TransferPanelViewModel {
        let tasks = session_id
            .map(|id| service.transfer_snapshots(id))
            .unwrap_or_default();

        let (active, completed, failed) =
            tasks.iter().fold((0, 0, 0), |(a, c, f), t| match t.status {
                crate::model::TransferStatus::Queued | crate::model::TransferStatus::Running => {
                    (a + 1, c, f)
                }
                crate::model::TransferStatus::Completed => (a, c + 1, f),
                crate::model::TransferStatus::Failed => (a, c, f + 1),
                _ => (a, c, f),
            });

        TransferPanelViewModel {
            active_count: active,
            completed_count: completed,
            failed_count: failed,
            rows: tasks
                .into_iter()
                .map(|t| TransferRowViewModel {
                    id: t.id,
                    direction_icon: match t.direction {
                        crate::model::TransferDirection::Upload => "\u{2191}",
                        crate::model::TransferDirection::Download => "\u{2193}",
                    },
                    file_name: std::path::Path::new(&t.remote_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| t.remote_path.clone()),
                    progress_percent: if t.total_bytes > 0 {
                        ((t.transferred_bytes as f64 / t.total_bytes as f64) * 100.0) as u8
                    } else {
                        0
                    },
                    status_text: match t.status {
                        crate::model::TransferStatus::Queued => "排队中".into(),
                        crate::model::TransferStatus::Running => "传输中".into(),
                        crate::model::TransferStatus::Completed => "完成".into(),
                        crate::model::TransferStatus::Failed => "失败".into(),
                        crate::model::TransferStatus::Cancelled => "已取消".into(),
                    },
                    status_color: match t.status {
                        crate::model::TransferStatus::Queued => hsla(0.0, 0.0, 0.5, 1.0),
                        crate::model::TransferStatus::Running => hsla(0.55, 0.8, 0.5, 1.0),
                        crate::model::TransferStatus::Completed => hsla(0.4, 0.8, 0.5, 1.0),
                        crate::model::TransferStatus::Failed => hsla(0.0, 0.8, 0.5, 1.0),
                        crate::model::TransferStatus::Cancelled => hsla(0.12, 0.7, 0.5, 1.0),
                    },
                    speed_text: String::new(),
                    logs: t.logs,
                    expanded: false,
                })
                .collect(),
        }
    }
}

impl SshViewModel {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            sessions: Vec::new(),
            file_tree: FileTreeViewModel {
                current_path: String::new(),
                parent_path: None,
                entries: Vec::new(),
            },
            terminal: TerminalViewModel {
                status: "未连接".into(),
                lines: Vec::new(),
                cursor_visible: false,
                cursor_row: 0,
                cursor_col: 0,
                terminal_kind: TerminalKind::Shell,
            },
            transfers: TransferPanelViewModel {
                active_count: 0,
                completed_count: 0,
                failed_count: 0,
                rows: Vec::new(),
            },
        }
    }
}

// ========== Render ==========

impl Render for SshView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let handle = cx.entity().clone();
        let overlay_handle = handle.clone();
        div()
            .size_full()
            .relative()
            // 主内容区域（flex 布局）
            .child(
                div()
                    .size_full()
                    .flex()
                    // 左侧列
                    .child(sidebar::render_sidebar(
                        &self.vm.profiles,
                        self.selected_profile_id,
                        self.context_menu_profile_id,
                        self.context_menu_position,
                        cx,
                    ))
                    // 右侧列
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .h_full()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(session_tabs::render_session_tabs(&self.vm.sessions, cx))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .min_h(px(0.0))
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .min_h(px(0.0))
                                            .min_w(px(0.0))
                                            .overflow_hidden()
                                            .child(file_tree::render_file_tree(
                                                &self.vm.file_tree,
                                                cx,
                                            ))
                                            .child(terminal_pane::render_terminal(
                                                &self.vm.terminal,
                                                self.terminal_font_size,
                                                &self.focus_handle,
                                                cx,
                                            )),
                                    )
                                    .child(transfer_panel::render_transfer_panel(
                                        &self.vm.transfers,
                                        self.transfer_panel_expanded,
                                        cx,
                                    )),
                            ),
                    ),
            )
            .when(self.show_profile_editor, {
                let editor_handle = overlay_handle.clone();
                let inputs = profile_editor::ProfileFormInputs {
                    name: self.form_name.clone(),
                    host: self.form_host.clone(),
                    port: self.form_port.clone(),
                    username: self.form_username.clone(),
                    password: self.form_password.clone(),
                    remote_root: self.form_remote_root.clone(),
                    local_root: self.form_local_root.clone(),
                    private_key_path: self.form_private_key_path.clone(),
                    private_key_passphrase: self.form_private_key_passphrase.clone(),
                    note: self.form_note.clone(),
                    connection_timeout: self.form_connection_timeout.clone(),
                    keepalive_interval: self.form_keepalive_interval.clone(),
                };
                let protocol = self.form_protocol.clone();
                let auth_method = self.form_auth_method.clone();
                let advanced_expanded = self.form_advanced_expanded;
                let is_edit = self.editing_profile_id.is_some();
                move |root| {
                    root.child(profile_editor::render_profile_editor(
                        editor_handle.clone(),
                        &inputs,
                        &protocol,
                        &auth_method,
                        advanced_expanded,
                        is_edit,
                    ))
                }
            })
            .when(self.show_app_settings, {
                let settings_handle = overlay_handle.clone();
                let inputs = app_settings::AppSettingsInputs {
                    terminal_font_size: self.form_terminal_font_size.clone(),
                };
                let font_size = self.terminal_font_size;
                move |root| {
                    root.child(app_settings::render_app_settings(
                        settings_handle.clone(),
                        &inputs,
                        font_size,
                    ))
                }
            })
            .when(self.context_menu_profile_id.is_some(), {
                let menu_handle = overlay_handle.clone();
                let profile_id = self.context_menu_profile_id.unwrap_or(0);
                let position = self
                    .context_menu_position
                    .unwrap_or(point(px(120.0), px(120.0)));
                move |root| {
                    root.child(context_menu::render_profile_context_menu(
                        menu_handle.clone(),
                        profile_id,
                        position,
                    ))
                }
            })
            .when(self.file_context_menu_position.is_some() && self.selected_session_id.is_some(), {
                let menu_handle = overlay_handle.clone();
                let entry = self.file_context_menu_entry.clone();
                let position = self
                    .file_context_menu_position
                    .unwrap_or(point(px(200.0), px(200.0)));
                move |root| {
                    root.child(file_context_menu::render_file_context_menu(
                        menu_handle.clone(),
                        entry.clone(),
                        position,
                    ))
                }
            })
    }
}

impl Focusable for SshView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

//! SshView 主视图 + ViewModel + 布局

mod app_settings;
mod aux_windows;
mod context_menu;
mod file_context_menu;
mod file_edit_confirm;
mod file_rename;
mod file_upload_overwrite;
mod file_tree;
mod profile_editor;
mod session_tabs;
mod sidebar;
mod terminal_element;
mod terminal_input;
mod terminal_pane;
mod terminal_pane_view;
mod terminal_shell_element;
mod transfer_panel;
mod virtual_list;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;

use crate::model::{AuthConfig, ProtocolType, SessionId, SessionStatus, SshAuthMethod, TerminalKind};
use crate::service::{SshEvent, SshService};
use crate::terminal::{TerminalCell, TerminalLine, TerminalModes};
use terminal_input::terminal_editing_key;
use terminal_pane_view::TerminalPaneView;
use crate::transfer;
use crate::upload::{self, UploadItem};
use gpui::prelude::FluentBuilder;
use gpui::*;
use qingqi_ui::text_input::{TextInput, TextInputStyle};
use qingqi_ui::theme;

actions!(ssh_terminal, [SshTerminalTab, SshTerminalShiftTab]);

static SSH_TERMINAL_KEYBINDINGS: OnceLock<()> = OnceLock::new();

fn ensure_ssh_terminal_keybindings(cx: &mut App) {
    if SSH_TERMINAL_KEYBINDINGS.set(()).is_ok() {
        cx.bind_keys([
            KeyBinding::new("tab", SshTerminalTab, Some("ssh_terminal")),
            KeyBinding::new("shift-tab", SshTerminalShiftTab, Some("ssh_terminal")),
        ]);
    }
}

// ========== ViewModel (render-ready 纯数据) ==========

#[derive(Clone, Debug)]
pub struct ProfileItem {
    pub id: i64,
    pub name: String,
    pub endpoint: String,
    pub protocol_badge: String,
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
    pub grid: Vec<Vec<TerminalCell>>,
    pub cols: usize,
    pub rows: usize,
    pub display_offset: usize,
    pub max_display_offset: usize,
    pub cursor_visible: bool,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub modes: TerminalModes,
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

#[derive(Clone, Debug)]
struct PendingExternalEdit {
    session_id: SessionId,
    remote_path: String,
    local_path: PathBuf,
    file_name: String,
}

#[derive(Clone, Debug)]
struct PendingUploadBatch {
    session_id: SessionId,
    items: Vec<UploadItem>,
    conflict_remotes: HashSet<String>,
}

// ========== SshView ==========

pub struct SshView {
    service: Arc<SshService>,
    focus_handle: FocusHandle,
    terminal_pane: Entity<TerminalPaneView>,

    vm: SshViewModel,

    selected_profile_id: Option<i64>,
    selected_session_id: Option<SessionId>,
    transfer_panel_expanded: bool,
    profile_editor_window: Option<AnyWindowHandle>,
    app_settings_window: Option<AnyWindowHandle>,
    editing_profile_id: Option<i64>, // None=新建, Some(id)=编辑
    form_advanced_expanded: bool,
    show_file_rename: bool,
    file_rename_input: Entity<TextInput>,
    file_rename_target: Option<FileEntryRow>,
    show_file_upload_confirm: bool,
    pending_external_edit: Option<PendingExternalEdit>,
    show_upload_overwrite_confirm: bool,
    pending_upload_batch: Option<PendingUploadBatch>,
    edit_watch_generation: u64,
    selected_file_path: Option<String>,
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
    form_keepalive_max: Entity<TextInput>,
    form_tcp_nodelay: bool,
    form_ftp_passive_mode: bool,
    form_ftp_passive_nat_workaround: bool,
    form_terminal_font_size: Entity<TextInput>,
    file_path_input: Entity<TextInput>,
    follow_terminal: bool,
    path_input_session: Option<SessionId>,
    last_seen_shell_cwd: Option<String>,
    terminal_input_buffer: String,
    terminal_line_tracking: bool,
    last_terminal_grid: Option<(usize, usize)>,
    file_list_scroll: UniformListScrollHandle,
    profile_list_scroll: UniformListScrollHandle,
    transfer_list_scroll: UniformListScrollHandle,
    session_tab_scroll: ScrollHandle,
    /// 文件列表右键菜单目标（`None` = 空白区域）
    file_context_target: Option<FileEntryRow>,

    event_task: Option<Task<()>>,
    generation: u64,
    profiles_cache: Vec<crate::model::Profile>,
}

fn form_input(
    cx: &mut Context<TextInput>,
    placeholder: impl Into<SharedString>,
    value: impl Into<SharedString>,
) -> TextInput {
    let mut input = TextInput::new(cx, placeholder, value);
    input.set_chrome(true, cx);
    input.set_style(
        TextInputStyle {
            height: 32.0,
            font_size: 13.0,
            padding: 5.0,
        },
        cx,
    );
    input
}

fn path_input(cx: &mut Context<TextInput>) -> TextInput {
    let mut input = TextInput::new(cx, "/remote/path", "");
    input.set_chrome(false, cx);
    input.set_monospace(true, cx);
    input.set_style(
        TextInputStyle {
            height: 24.0,
            font_size: 11.0,
            padding: 0.0,
        },
        cx,
    );
    input.set_text_colors(
        theme::semantic().text_primary,
        theme::semantic().text_secondary,
        cx,
    );
    input
}

impl SshView {
    pub fn new(service: Arc<SshService>, cx: &mut Context<Self>) -> Self {
        ensure_ssh_terminal_keybindings(cx);
        let service_for_pane = Arc::clone(&service);
        let ssh_entity = cx.entity().clone();
        let terminal_focus = cx.focus_handle().tab_stop(false);
        let terminal_pane = cx.new(|cx| {
            TerminalPaneView::new(
                ssh_entity,
                service_for_pane,
                12.0,
                terminal_focus,
                cx,
            )
        });
        let mut this = Self {
            service: Arc::clone(&service),
            focus_handle: cx.focus_handle(),
            terminal_pane,
            vm: SshViewModel::default(),
            selected_profile_id: None,
            selected_session_id: None,
            transfer_panel_expanded: false,
            profile_editor_window: None,
            app_settings_window: None,
            editing_profile_id: None,
            form_advanced_expanded: false,
            show_file_rename: false,
            file_rename_input: cx.new(|cx| {
                let mut input = form_input(cx, "新名称", "");
                input.set_style(
                    TextInputStyle {
                        height: 32.0,
                        font_size: 13.0,
                        padding: 6.0,
                    },
                    cx,
                );
                input
            }),
            file_rename_target: None,
            show_file_upload_confirm: false,
            pending_external_edit: None,
            show_upload_overwrite_confirm: false,
            pending_upload_batch: None,
            edit_watch_generation: 0,
            selected_file_path: None,
            terminal_font_size: 12.0,
            form_protocol: ProtocolType::Ssh,
            form_auth_method: SshAuthMethod::Password { password: String::new() },
            form_name: cx.new(|cx| form_input(cx, "连接名称", "")),
            form_host: cx.new(|cx| form_input(cx, "主机地址或 IP", "")),
            form_port: cx.new(|cx| form_input(cx, "22", "22")),
            form_username: cx.new(|cx| form_input(cx, "登录用户名", "root")),
            form_password: cx.new(|cx| form_input(cx, "登录密码", "")),
            form_remote_root: cx.new(|cx| form_input(cx, "如 ~ 或 /var/www", "~")),
            form_local_root: cx.new(|cx| form_input(cx, "本地下载目录", "~/Downloads")),
            form_private_key_path: cx.new(|cx| form_input(cx, "~/.ssh/id_rsa", "~/.ssh/id_rsa")),
            form_private_key_passphrase: cx.new(|cx| form_input(cx, "私钥密码（可选）", "")),
            form_note: cx.new(|cx| {
                let mut input = form_input(cx, "备注说明", "");
                input.set_multiline(true, cx);
                input.set_style(
                    TextInputStyle {
                        height: 64.0,
                        font_size: 13.0,
                        padding: 6.0,
                    },
                    cx,
                );
                input
            }),
            form_connection_timeout: cx.new(|cx| form_input(cx, "0", "0")),
            form_keepalive_interval: cx.new(|cx| form_input(cx, "60", "60")),
            form_keepalive_max: cx.new(|cx| form_input(cx, "3", "3")),
            form_tcp_nodelay: false,
            form_ftp_passive_mode: true,
            form_ftp_passive_nat_workaround: true,
            form_terminal_font_size: cx.new(|cx| form_input(cx, "12", "12")),
            file_path_input: cx.new(|cx| path_input(cx)),
            follow_terminal: false,
            path_input_session: None,
            last_seen_shell_cwd: None,
            terminal_input_buffer: String::new(),
            terminal_line_tracking: true,
            last_terminal_grid: None,
            file_list_scroll: UniformListScrollHandle::new(),
            profile_list_scroll: UniformListScrollHandle::new(),
            transfer_list_scroll: UniformListScrollHandle::new(),
            session_tab_scroll: ScrollHandle::new(),
            file_context_target: None,
            event_task: None,
            generation: 0,
            profiles_cache: Vec::new(),
        };
        this.rebuild_view_model(cx);
        this.start_event_loop(cx);
        this
    }

    fn start_event_loop(&mut self, cx: &mut Context<Self>) {
        let service = Arc::clone(&self.service);
        let mut rx = service.subscribe();

        self.generation = self.generation.wrapping_add(1);
        let task_gen = self.generation;

        self.event_task = Some(cx.spawn(async move |view, acx| {
            loop {
                match rx.recv().await {
                    Ok(first) => {
                        let mut batch = vec![first];
                        while let Ok(event) = rx.try_recv() {
                            batch.push(event);
                        }
                        let _ = view.update(acx, |view, cx| {
                            if view.generation != task_gen {
                                return;
                            }
                            view.on_service_events(&batch, cx);
                        });
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(
                            target: "qingqi_ssh",
                            skipped = n,
                            "term_diag: 事件广播滞后，强制全量刷新"
                        );
                        let _ = view.update(acx, |view, cx| {
                            if view.generation != task_gen {
                                return;
                            }
                            view.refresh_ui(cx);
                        });
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }));
    }

    fn on_service_events(&mut self, events: &[SshEvent], cx: &mut Context<Self>) {
        tracing::debug!(target: "qingqi_ssh", count = events.len(), "view: service events");
        for event in events {
            match event {
                SshEvent::ProfileCreated(_)
                | SshEvent::ProfileUpdated(_)
                | SshEvent::ProfileDeleted(_) => self.profiles_cache.clear(),
                _ => {}
            }
        }
        let terminal_only = events
            .iter()
            .all(|event| matches!(event, SshEvent::SessionDataChanged(_)));
        if events.iter().any(|event| {
            matches!(
                event,
                SshEvent::SessionConnected(_) | SshEvent::SessionDisconnected(_)
            )
        }) {
            self.last_terminal_grid = None;
        }
        if terminal_only {
            if self.file_tree_out_of_sync() {
                tracing::debug!(
                    target: "qingqi_ssh",
                    "view: 文件树与 service 不同步，终端事件中补全量刷新"
                );
                self.ensure_selected_session_file_list();
                self.rebuild_view_model(cx);
                self.sync_file_path_input_if_needed(cx);
                cx.notify();
                return;
            }
            self.refresh_terminal_only(cx);
            cx.notify();
            return;
        }
        if events.iter().any(|event| matches!(event, SshEvent::TransferChanged(_, _))) {
            self.rebuild_view_model(cx);
            cx.notify();
            return;
        }
        for event in events {
            if let SshEvent::SessionConnected(sid) = event {
                tracing::debug!(
                    target: "qingqi_ssh",
                    session_id = %sid.0,
                    selected = ?self.selected_session_id,
                    service_entries = self.service.session_entries(sid).len(),
                    "SessionConnected"
                );
                if self.selected_session_id.is_none() {
                    self.selected_session_id = Some(*sid);
                }
                if self.selected_session_id == Some(*sid) {
                    self.ensure_selected_session_file_list();
                }
            }
        }
        self.rebuild_view_model(cx);
        self.sync_file_path_input_if_needed(cx);
        tracing::debug!(
            target: "qingqi_ssh",
            file_entries = self.vm.file_tree.entries.len(),
            file_path = %self.vm.file_tree.current_path,
            "view: on_service_events 完成"
        );
        cx.notify();
    }

    /// 当前选中 Session 在 service 中已有目录缓存，但 ViewModel 未跟上。
    fn file_tree_out_of_sync(&self) -> bool {
        let Some(sid) = self.selected_session_id else {
            return false;
        };
        let connected = self
            .service
            .session_summary(&sid)
            .is_some_and(|summary| summary.status == SessionStatus::Connected);
        if !connected {
            return false;
        }
        let service_count = self.service.session_entries(&sid).len();
        if service_count == 0 {
            return false;
        }
        let vm_files = self
            .vm
            .file_tree
            .entries
            .iter()
            .filter(|entry| !entry.is_parent)
            .count();
        vm_files != service_count
            || self.vm.file_tree.current_path != self.service.session_cwd(&sid)
    }

    fn sync_terminal_pane(&mut self, cx: &mut Context<Self>) {
        self.terminal_pane.update(cx, |pane, cx| {
            pane.sync_from_parent(
                self.selected_session_id,
                self.vm.terminal.clone(),
                self.terminal_font_size,
                self.transfer_panel_expanded,
            );
            cx.notify();
        });
    }

    /// 仅刷新终端画面，PTY 高频输出时避免重建侧栏/文件树。
    fn refresh_terminal_only(&mut self, cx: &mut Context<Self>) {
        let terminal = Self::build_terminal(self.selected_session_id.as_ref(), &self.service);
        tracing::debug!(
            target: "qingqi_ssh",
            rows = terminal.rows,
            cols = terminal.cols,
            grid_rows = terminal.grid.len(),
            display_offset = terminal.display_offset,
            max_display_offset = terminal.max_display_offset,
            cursor_row = terminal.cursor_row,
            cursor_col = terminal.cursor_col,
            cursor_visible = terminal.cursor_visible,
            "term_diag: refresh_terminal_only"
        );
        self.vm.terminal = terminal.clone();
        self.terminal_pane.update(cx, |pane, cx| {
            pane.sync_from_parent(
                self.selected_session_id,
                terminal,
                self.terminal_font_size,
                self.transfer_panel_expanded,
            );
            cx.notify();
        });
    }

    /// 从 Service 拉取最新数据并通知 GPUI 重绘
    fn refresh_ui(&mut self, cx: &mut Context<Self>) {
        self.rebuild_view_model(cx);
        self.sync_file_path_input_if_needed(cx);
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

    pub(crate) fn terminal_input_enabled(&self) -> bool {
        self.profile_editor_window.is_none() && self.app_settings_window.is_none()
    }

    /// 跟踪 shell 输入行（follow_terminal cd 解析），PTY 写入由 TerminalPaneView 负责。
    pub(crate) fn track_terminal_input(
        &mut self,
        sid: &SessionId,
        bytes: &[u8],
        cx: &mut Context<Self>,
    ) {
        if bytes == b"\r" {
            if self.follow_terminal
                && self.terminal_line_tracking
                && matches!(self.vm.terminal.terminal_kind, TerminalKind::Shell)
            {
                let basis = self.service.shell_cwd_basis(sid);
                if let Some(target) =
                    crate::shell_cwd::resolve_cd_command(&self.terminal_input_buffer, &basis)
                {
                    self.apply_follow_cd(sid, &target, cx);
                }
            }
            self.terminal_input_buffer.clear();
            self.terminal_line_tracking = true;
        } else if bytes == b"\x7f" || bytes == b"\x08" {
            if self.terminal_line_tracking {
                self.terminal_input_buffer.pop();
            }
        } else if bytes.len() == 1 && bytes[0] >= 0x20 && bytes[0] != 0x7f {
            if self.terminal_line_tracking {
                self.terminal_input_buffer.push(bytes[0] as char);
            }
        } else if let Ok(text) = std::str::from_utf8(bytes) {
            if self.terminal_line_tracking
                && !text.is_empty()
                && text.chars().all(|c| !c.is_control())
            {
                self.terminal_input_buffer.push_str(text);
            }
        } else if terminal_editing_key(bytes) {
            self.terminal_line_tracking = false;
            self.terminal_input_buffer.clear();
        }
    }

    fn load_profiles_cache(&mut self) {
        if self.profiles_cache.is_empty() {
            self.profiles_cache = self.service.list_profiles().unwrap_or_default();
        }
    }

    fn apply_follow_cd(&mut self, sid: &SessionId, target: &str, cx: &mut Context<Self>) {
        match self.service.list_directory(sid, target) {
            Ok(_) => {
                self.service.set_session_shell_cwd(sid, target);
                self.last_seen_shell_cwd = Some(target.to_string());
                self.sync_file_path_input(cx, target);
            }
            Err(error) => {
                tracing::debug!(
                    target: "qingqi_ssh",
                    path = target,
                    error = %error,
                    "跟随终端切换目录失败"
                );
            }
        }
    }

    // ===== 用户交互 =====

    /// 单击选中 Profile
    pub(crate) fn select_profile(&mut self, profile_id: i64, cx: &mut Context<Self>) {
        self.selected_profile_id = Some(profile_id);
        self.refresh_ui(cx);
    }

    /// 双击 / 右键菜单连接
    pub(crate) fn connect_profile(&mut self, profile_id: i64, cx: &mut Context<Self>) {
        self.selected_profile_id = Some(profile_id);
        tracing::debug!(
            target: "qingqi_ssh",
            profile_id,
            "用户发起连接"
        );
        match self.service.open_session(profile_id) {
            Ok(session_id) => {
                self.selected_session_id = Some(session_id);
                self.last_terminal_grid = None;
            }
            Err(e) => {
                tracing::error!(target: "qingqi_ssh", profile_id, error = %e, "连接发起失败");
            }
        }
        self.refresh_ui(cx);
    }

    fn select_session(&mut self, session_id: SessionId, cx: &mut Context<Self>) {
        self.selected_session_id = Some(session_id);
        self.selected_file_path = None;
        self.last_terminal_grid = None;
        self.path_input_session = None;
        self.last_seen_shell_cwd = None;
        self.terminal_input_buffer.clear();
        self.terminal_line_tracking = true;
        self.ensure_selected_session_file_list();
        self.refresh_ui(cx);
    }

    /// 已连接但目录缓存为空时补拉文件列表（切换 Tab、连接完成等场景）。
    fn ensure_selected_session_file_list(&mut self) {
        let Some(sid) = self.selected_session_id else {
            return;
        };
        let connected = self
            .service
            .session_summary(&sid)
            .is_some_and(|summary| summary.status == SessionStatus::Connected);
        if !connected || !self.service.session_entries(&sid).is_empty() {
            return;
        }
        let cwd = self.service.session_cwd(&sid);
        if cwd.is_empty() {
            return;
        }
        if let Err(error) = self.service.list_directory(&sid, &cwd) {
            tracing::warn!(
                target: "qingqi_ssh",
                session_id = %sid.0,
                path = %cwd,
                error = %error,
                "补拉文件列表失败"
            );
        }
    }

    fn sync_file_path_input(&mut self, cx: &mut Context<Self>, path: &str) {
        self.file_path_input.update(cx, |input, cx| {
            input.set_text(path, cx);
        });
    }

    fn sync_file_path_input_if_needed(&mut self, cx: &mut Context<Self>) {
        if self.path_input_session != self.selected_session_id {
            self.path_input_session = self.selected_session_id;
            let path = self.vm.file_tree.current_path.clone();
            self.sync_file_path_input(cx, &path);
        }
    }

    fn mark_manual_path_navigation(&mut self, path: &str) {
        let Some(sid) = self.selected_session_id else {
            return;
        };
        self.service.set_session_shell_cwd(&sid, path);
        self.last_seen_shell_cwd = Some(path.to_string());
    }

    pub(crate) fn jump_to_path_in_input(&mut self, cx: &mut Context<Self>) {
        let Some(sid) = self.selected_session_id else {
            return;
        };
        let path = self.file_path_input.read(cx).text();
        let path = path.trim();
        if path.is_empty() {
            return;
        }
        match self.service.list_directory(&sid, path) {
            Ok(_) => {
                self.mark_manual_path_navigation(path);
                self.sync_file_path_input(cx, path);
                self.refresh_ui(cx);
            }
            Err(error) => tracing::warn!(target: "qingqi_ssh", path, error = %error, "跳转目录失败"),
        }
    }

    pub(crate) fn toggle_follow_terminal(&mut self, cx: &mut Context<Self>) {
        self.follow_terminal = !self.follow_terminal;
        if self.follow_terminal {
            if let Some(sid) = self.selected_session_id {
                let cwd = self.service.session_cwd(&sid);
                self.service.set_session_shell_cwd(&sid, &cwd);
                self.last_seen_shell_cwd = Some(cwd);
            }
        }
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
        self.selected_file_path = None;
        match self.service.list_directory(&sid, path) {
            Ok(_) => {
                self.mark_manual_path_navigation(path);
                self.sync_file_path_input(cx, path);
                self.refresh_ui(cx);
            }
            Err(e) => tracing::warn!(target: "qingqi_ssh", path, error = %e, "进入目录失败"),
        }
    }

    /// 双击文件列表项
    pub(crate) fn open_file_entry(&mut self, entry: &FileEntryRow, cx: &mut Context<Self>) {
        if entry.is_parent {
            self.selected_file_path = None;
            self.navigate_to_remote_path(&entry.path, cx);
        } else if entry.is_dir {
            self.selected_file_path = None;
            self.navigate_to_remote_path(&entry.path, cx);
        } else {
            self.selected_file_path = Some(entry.path.clone());
            self.open_file_editor(entry, cx);
        }
    }

    pub(crate) fn select_file_entry(&mut self, entry: &FileEntryRow, cx: &mut Context<Self>) {
        self.selected_file_path = if entry.is_dir || entry.is_parent {
            None
        } else {
            Some(entry.path.clone())
        };
        self.rebuild_view_model(cx);
        cx.notify();
    }

    pub(crate) fn close_session(&mut self, session_id: SessionId, cx: &mut Context<Self>) {
        if self.selected_session_id == Some(session_id) {
            self.selected_session_id = self
                .service
                .session_summaries()
                .into_iter()
                .map(|summary| summary.session_id)
                .filter(|id| *id != session_id)
                .next_back();
        }
        let _ = self.service.close_session(&session_id);
        self.ensure_selected_session_file_list();
        self.refresh_ui(cx);
    }

    fn toggle_transfer_panel(&mut self, cx: &mut Context<Self>) {
        self.transfer_panel_expanded = !self.transfer_panel_expanded;
        cx.notify();
    }

    pub(crate) fn set_file_context_target(&mut self, target: Option<FileEntryRow>) {
        self.file_context_target = target;
    }

    pub(crate) fn take_file_context_target(&mut self) -> Option<FileEntryRow> {
        self.file_context_target.take()
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
        if entry.is_dir && !entry.is_parent {
            let local_dir = self.local_download_path(cx).join(&entry.name);
            match crate::download::collect_download_items(
                &self.service,
                &sid,
                &entry.path,
                &local_dir,
            ) {
                Ok(items) if items.is_empty() => {}
                Ok(items) => {
                    for item in items {
                        if let Err(error) =
                            self.service.download_file(&sid, &item.remote, &item.local)
                        {
                            tracing::warn!(
                                target: "qingqi_ssh",
                                remote = %item.remote,
                                error = %error,
                                "下载失败"
                            );
                        }
                    }
                    self.transfer_panel_expanded = true;
                    self.refresh_ui(cx);
                }
                Err(error) => {
                    tracing::warn!(target: "qingqi_ssh", error = %error, "收集下载任务失败")
                }
            }
            return;
        }
        let local = self.local_download_path(cx).join(&entry.name);
        match self.service.download_file(&sid, &entry.path, &local) {
            Ok(_) => {
                self.transfer_panel_expanded = true;
                self.refresh_ui(cx);
            }
            Err(error) => tracing::warn!(target: "qingqi_ssh", error = %error, "下载失败"),
        }
    }

    pub(crate) fn upload_local_paths(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        let Some(sid) = self.selected_session_id else {
            return;
        };
        let cwd = self.service.session_cwd(&sid);
        if cwd.is_empty() {
            return;
        }
        if paths.is_empty() {
            return;
        }

        let paths: Vec<PathBuf> = paths.to_vec();
        let service = Arc::clone(&self.service);
        cx.spawn(async move |view, cx| {
            let collect = cx
                .background_executor()
                .spawn(async move { upload::collect_upload_items(&paths, &cwd) })
                .await;

            let items = match collect {
                Ok(items) if !items.is_empty() => items,
                Ok(_) => return,
                Err(error) => {
                    let _ = view.update(cx, |_, _| {
                        tracing::warn!(target: "qingqi_ssh", error = %error, "收集上传任务失败");
                    });
                    return;
                }
            };

            let conflicts = cx
                .background_executor()
                .spawn({
                    let items_for_check = items.clone();
                    async move { upload::find_upload_conflicts(&service, &sid, &items_for_check) }
                })
                .await;

            let _ = view.update(cx, |view, cx| {
                match conflicts {
                    Ok(conflicts) if conflicts.is_empty() => {
                        view.run_upload_batch(sid, items, HashSet::new(), true, cx);
                    }
                    Ok(conflicts) => {
                        let conflict_remotes = conflicts
                            .into_iter()
                            .map(|item| item.remote)
                            .collect();
                        view.pending_upload_batch = Some(PendingUploadBatch {
                            session_id: sid,
                            items,
                            conflict_remotes,
                        });
                        view.show_upload_overwrite_confirm = true;
                        cx.notify();
                    }
                    Err(error) => {
                        tracing::warn!(target: "qingqi_ssh", error = %error, "检测上传冲突失败");
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn pick_and_upload_files(&mut self, cx: &mut Context<Self>) {
        if self.selected_session_id.is_none() {
            return;
        }
        match qingqi_platform::shell::choose_file("选择要上传的文件") {
            Ok(Some(path)) => self.upload_local_paths(&[path], cx),
            Ok(None) => {}
            Err(error) => tracing::warn!(target: "qingqi_ssh", error = %error, "打开文件选择器失败"),
        }
    }

    pub(crate) fn pick_and_upload_folder(&mut self, cx: &mut Context<Self>) {
        if self.selected_session_id.is_none() {
            return;
        }
        match qingqi_platform::shell::choose_directory("选择要上传的文件夹") {
            Ok(Some(path)) => self.upload_local_paths(&[path], cx),
            Ok(None) => {}
            Err(error) => tracing::warn!(target: "qingqi_ssh", error = %error, "打开文件夹选择器失败"),
        }
    }

    pub(crate) fn confirm_pending_upload(&mut self, replace_existing: bool, cx: &mut Context<Self>) {
        let Some(batch) = self.pending_upload_batch.take() else {
            self.show_upload_overwrite_confirm = false;
            cx.notify();
            return;
        };
        self.show_upload_overwrite_confirm = false;
        self.run_upload_batch(
            batch.session_id,
            batch.items,
            batch.conflict_remotes,
            replace_existing,
            cx,
        );
    }

    pub(crate) fn cancel_pending_upload(&mut self, cx: &mut Context<Self>) {
        self.show_upload_overwrite_confirm = false;
        self.pending_upload_batch = None;
        cx.notify();
    }

    fn run_upload_batch(
        &mut self,
        session_id: SessionId,
        items: Vec<UploadItem>,
        conflict_remotes: HashSet<String>,
        replace_existing: bool,
        cx: &mut Context<Self>,
    ) {
        let mut started = false;
        for item in items {
            if !replace_existing && conflict_remotes.contains(&item.remote) {
                continue;
            }
            if let Err(error) = self
                .service
                .ensure_remote_parent_dirs(&session_id, &item.remote)
            {
                tracing::warn!(
                    target: "qingqi_ssh",
                    remote = %item.remote,
                    error = %error,
                    "创建远程目录失败"
                );
                continue;
            }
            match self.service.upload_file(&session_id, &item.local, &item.remote) {
                Ok(_) => started = true,
                Err(error) => tracing::warn!(
                    target: "qingqi_ssh",
                    path = %item.local.display(),
                    error = %error,
                    "上传失败"
                ),
            }
        }
        if started {
            self.transfer_panel_expanded = true;
            let cwd = self.service.session_cwd(&session_id);
            if !cwd.is_empty() {
                let _ = self.service.list_directory(&session_id, &cwd);
            }
            self.refresh_ui(cx);
        }
    }

    pub(crate) fn open_file_rename(&mut self, entry: &FileEntryRow, cx: &mut Context<Self>) {
        if entry.is_parent {
            return;
        }
        self.file_rename_target = Some(entry.clone());
        self.file_rename_input
            .update(cx, |input, cx| input.set_text(&entry.name, cx));
        self.show_file_rename = true;
        cx.notify();
    }

    pub(crate) fn close_file_rename(&mut self, cx: &mut Context<Self>) {
        self.show_file_rename = false;
        self.file_rename_target = None;
        cx.notify();
    }

    pub(crate) fn confirm_file_rename(&mut self, cx: &mut Context<Self>) {
        let Some(entry) = self.file_rename_target.clone() else {
            self.close_file_rename(cx);
            return;
        };
        let new_name = self.file_rename_input.read(cx).current_text(cx).trim().to_string();
        if new_name.is_empty() || new_name == entry.name {
            self.close_file_rename(cx);
            return;
        }
        let Some(sid) = self.selected_session_id else {
            self.close_file_rename(cx);
            return;
        };
        let parent = entry
            .path
            .trim_end_matches(&entry.name)
            .trim_end_matches('/');
        let new_path = if parent.is_empty() {
            new_name.clone()
        } else {
            format!("{parent}/{new_name}")
        };
        match self.service.rename_remote_entry(&sid, &entry.path, &new_path) {
            Ok(()) => {
                let cwd = self.service.session_cwd(&sid);
                if !cwd.is_empty() {
                    let _ = self.service.list_directory(&sid, &cwd);
                }
                self.refresh_ui(cx);
            }
            Err(error) => tracing::warn!(target: "qingqi_ssh", error = %error, "重命名失败"),
        }
        self.close_file_rename(cx);
    }

    pub(crate) fn delete_file_entry(&mut self, entry: &FileEntryRow, cx: &mut Context<Self>) {
        if entry.is_parent {
            return;
        }
        let Some(sid) = self.selected_session_id else {
            return;
        };
        match self
            .service
            .remove_remote_entry(&sid, &entry.path, entry.is_dir)
        {
            Ok(()) => {
                let cwd = self.service.session_cwd(&sid);
                if !cwd.is_empty() {
                    let _ = self.service.list_directory(&sid, &cwd);
                }
                self.refresh_ui(cx);
            }
            Err(error) => tracing::warn!(target: "qingqi_ssh", error = %error, "删除失败"),
        }
    }

    pub(crate) fn open_file_editor(&mut self, entry: &FileEntryRow, cx: &mut Context<Self>) {
        if entry.is_dir || entry.is_parent {
            return;
        }
        let Some(sid) = self.selected_session_id else {
            return;
        };
        self.show_file_upload_confirm = false;
        self.pending_external_edit = None;
        self.edit_watch_generation = self.edit_watch_generation.wrapping_add(1);
        let watch_gen = self.edit_watch_generation;

        let entry = entry.clone();
        let service = Arc::clone(&self.service);

        cx.spawn(async move |view, cx| {
            let local_path = service.edit_temp_path(&sid, &entry.name);
            let remote_path = entry.path.clone();
            let download = cx
                .background_executor()
                .spawn({
                    let local = local_path.clone();
                    async move { service.download_file_local(&sid, &remote_path, &local) }
                })
                .await;

            let _ = view.update(cx, |view, cx| {
                if view.edit_watch_generation != watch_gen {
                    return;
                }
                match download {
                    Ok(()) => {
                        let baseline = std::fs::metadata(&local_path)
                            .ok()
                            .and_then(|meta| meta.modified().ok());
                        match qingqi_platform::shell::open_path(&local_path) {
                            Ok(()) => {
                                view.pending_external_edit = Some(PendingExternalEdit {
                                    session_id: sid,
                                    remote_path: entry.path.clone(),
                                    local_path: local_path.clone(),
                                    file_name: entry.name.clone(),
                                });
                                view.start_external_edit_watch(watch_gen, baseline, cx);
                                cx.notify();
                            }
                            Err(error) => {
                                tracing::warn!(
                                    target: "qingqi_ssh",
                                    error = %error,
                                    "无法用系统编辑器打开文件"
                                );
                            }
                        }
                    }
                    Err(error) => {
                        tracing::warn!(target: "qingqi_ssh", error = %error, "下载文件到临时目录失败");
                    }
                }
            });
        })
        .detach();
    }

    fn start_external_edit_watch(
        &mut self,
        watch_gen: u64,
        baseline: Option<SystemTime>,
        cx: &mut Context<Self>,
    ) {
        let Some(pending) = self.pending_external_edit.clone() else {
            return;
        };
        let local_path = pending.local_path.clone();
        let handle = cx.entity().clone();

        cx.spawn(async move |view, cx| {
            let mut last_mtime = baseline;
            let mut stable_ticks = 0u32;
            let mut ever_modified = false;

            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;

                let stop = view
                    .update(cx, |view, _| {
                        view.edit_watch_generation != watch_gen
                            || view.pending_external_edit.is_none()
                            || view.show_file_upload_confirm
                    })
                    .unwrap_or(true);
                if stop {
                    break;
                }

                let current = std::fs::metadata(&local_path)
                    .ok()
                    .and_then(|meta| meta.modified().ok());

                if current != baseline {
                    ever_modified = true;
                }
                if !ever_modified {
                    continue;
                }

                if current == last_mtime {
                    stable_ticks += 1;
                    if stable_ticks >= 2 {
                        let _ = handle.update(cx, |view, cx| {
                            if view.edit_watch_generation != watch_gen {
                                return;
                            }
                            view.show_file_upload_confirm = true;
                            cx.notify();
                        });
                        break;
                    }
                } else {
                    stable_ticks = 0;
                    last_mtime = current;
                }
            }
        })
        .detach();
    }

    pub(crate) fn confirm_upload_external_edit(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending_external_edit.take() else {
            self.show_file_upload_confirm = false;
            cx.notify();
            return;
        };
        self.show_file_upload_confirm = false;
        self.edit_watch_generation = self.edit_watch_generation.wrapping_add(1);

        match self.service.upload_file(
            &pending.session_id,
            &pending.local_path,
            &pending.remote_path,
        ) {
            Ok(_) => {
                self.transfer_panel_expanded = true;
                self.refresh_ui(cx);
            }
            Err(error) => tracing::warn!(target: "qingqi_ssh", error = %error, "回传文件失败"),
        }
        cx.notify();
    }

    pub(crate) fn cancel_external_edit(&mut self, cx: &mut Context<Self>) {
        self.clear_external_edit_state(cx);
    }

    fn clear_external_edit_state(&mut self, cx: &mut Context<Self>) {
        self.show_file_upload_confirm = false;
        self.pending_external_edit = None;
        self.edit_watch_generation = self.edit_watch_generation.wrapping_add(1);
        cx.notify();
    }

    fn local_download_path(&self, cx: &Context<Self>) -> std::path::PathBuf {
        expand_user_path(&self.session_local_root(cx))
    }

    fn session_local_root(&self, cx: &Context<Self>) -> String {
        if let Some(sid) = self.selected_session_id {
            if let Some(summary) = self.service.session_summary(&sid) {
                if let Ok(Some(profile)) = self.service.get_profile(summary.profile_id) {
                    return profile.paths.local_root;
                }
            }
        }
        self.form_local_root.read(cx).text()
    }

    // ===== Profile 编辑弹窗 =====

    pub(crate) fn open_profile_editor(&mut self, profile_id: Option<i64>, cx: &mut Context<Self>) {
        self.close_app_settings(cx);
        self.close_profile_editor(cx);
        self.editing_profile_id = profile_id;
        self.form_advanced_expanded = false;

        if let Some(id) = profile_id {
            if let Ok(Some(profile)) = self.service.get_profile(id) {
                self.fill_form_from_profile(&profile, cx);
            }
        } else {
            self.reset_form(cx);
        }

        let is_edit = profile_id.is_some();
        let ssh_view = cx.entity().clone();
        cx.defer(move |cx| aux_windows::spawn_profile_editor_window(ssh_view, is_edit, cx));
        cx.notify();
    }

    pub(crate) fn close_profile_editor(&mut self, cx: &mut Context<Self>) {
        if self.profile_editor_window.is_none() {
            self.editing_profile_id = None;
            self.form_advanced_expanded = false;
            cx.notify();
            return;
        }
        aux_windows::close_window(&mut self.profile_editor_window, cx);
        self.form_advanced_expanded = false;
        cx.notify();
    }

    pub(crate) fn on_profile_editor_window_closed(
        &mut self,
        handle: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.profile_editor_window.as_ref() != Some(&handle) {
            return;
        }
        self.profile_editor_window = None;
        self.editing_profile_id = None;
        self.form_advanced_expanded = false;
        cx.notify();
    }

    pub(crate) fn toggle_form_advanced(&mut self, cx: &mut Context<Self>) {
        self.form_advanced_expanded = !self.form_advanced_expanded;
        cx.notify();
    }

    pub(crate) fn set_form_tcp_nodelay(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.form_tcp_nodelay == enabled {
            return;
        }
        self.form_tcp_nodelay = enabled;
        cx.notify();
    }

    pub(crate) fn set_form_ftp_passive_mode(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.form_ftp_passive_mode == enabled {
            return;
        }
        self.form_ftp_passive_mode = enabled;
        cx.notify();
    }

    pub(crate) fn set_form_ftp_passive_nat_workaround(
        &mut self,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if self.form_ftp_passive_nat_workaround == enabled {
            return;
        }
        self.form_ftp_passive_nat_workaround = enabled;
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
        self.form_keepalive_max.update(cx, |input, cx| {
            input.set_text(&profile.advanced.keepalive_max.to_string(), cx)
        });
        self.form_tcp_nodelay = profile.advanced.tcp_nodelay;
        self.form_ftp_passive_mode = profile.advanced.ftp_passive_mode;
        self.form_ftp_passive_nat_workaround = profile.advanced.ftp_passive_nat_workaround;
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
        self.form_connection_timeout.update(cx, |input, cx| input.set_text("0", cx));
        self.form_keepalive_interval.update(cx, |input, cx| input.set_text("60", cx));
        self.form_keepalive_max.update(cx, |input, cx| input.set_text("3", cx));
        self.form_tcp_nodelay = false;
        self.form_ftp_passive_mode = true;
        self.form_ftp_passive_nat_workaround = true;
    }

    // ===== 插件设置弹窗 =====

    pub(crate) fn open_app_settings(&mut self, cx: &mut Context<Self>) {
        self.close_profile_editor(cx);
        self.close_app_settings(cx);
        self.form_terminal_font_size.update(cx, |input, cx| {
            input.set_text(&self.terminal_font_size.round().to_string(), cx)
        });

        let ssh_view = cx.entity().clone();
        cx.defer(move |cx| aux_windows::spawn_app_settings_window(ssh_view, cx));
        cx.notify();
    }

    pub(crate) fn close_app_settings(&mut self, cx: &mut Context<Self>) {
        aux_windows::close_window(&mut self.app_settings_window, cx);
        cx.notify();
    }

    pub(crate) fn on_app_settings_window_closed(
        &mut self,
        handle: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.app_settings_window.as_ref() != Some(&handle) {
            return;
        }
        self.app_settings_window = None;
        cx.notify();
    }

    pub(crate) fn save_app_settings(&mut self, cx: &mut Context<Self>) {
        if let Ok(size) = self
            .form_terminal_font_size
            .read(cx)
            .current_text(cx)
            .parse::<f32>()
        {
            self.terminal_font_size = size.clamp(10.0, 20.0);
        }
        self.close_app_settings(cx);
        cx.notify();
    }

    fn set_form_protocol(&mut self, protocol: ProtocolType, cx: &mut Context<Self>) {
        let port = protocol.default_port();
        self.form_protocol = protocol;
        self.form_port.update(cx, |input, cx| {
            input.set_text(&port.to_string(), cx);
        });
        cx.notify();
    }

    fn set_form_auth_method(&mut self, method: SshAuthMethod, cx: &mut Context<Self>) {
        self.form_auth_method = method;
        cx.notify();
    }

    pub(crate) fn save_profile_from_form(&mut self, cx: &mut Context<Self>) {
        let editing_id = self.editing_profile_id;
        let field = |input: &Entity<TextInput>| input.read(cx).current_text(cx);
        let name = field(&self.form_name);
        let host = field(&self.form_host);
        let port: u16 = field(&self.form_port).parse().unwrap_or(22);
        let remote_root = field(&self.form_remote_root);
        let local_root = field(&self.form_local_root);
        let note = field(&self.form_note);
        let connection_timeout_secs = field(&self.form_connection_timeout)
            .parse()
            .unwrap_or(0);
        let keepalive_interval_secs = field(&self.form_keepalive_interval)
            .parse()
            .unwrap_or(60);
        let keepalive_max = field(&self.form_keepalive_max).parse().unwrap_or(3);
        let mut advanced = crate::model::ProfileAdvanced {
            connection_timeout_secs,
            keepalive_interval_secs,
            keepalive_max,
            tcp_nodelay: self.form_tcp_nodelay,
            ftp_passive_mode: self.form_ftp_passive_mode,
            ftp_passive_nat_workaround: self.form_ftp_passive_nat_workaround,
        };
        advanced.normalize_keepalive();

        if name.is_empty() || host.is_empty() {
            return;
        }

        let auth = match self.form_protocol {
            ProtocolType::Ftp | ProtocolType::Ftps => {
                let username = field(&self.form_username);
                let password = field(&self.form_password);
                AuthConfig::Ftp { username, password }
            }
            ProtocolType::Ssh => {
                let username = field(&self.form_username);
                let method = match &self.form_auth_method {
                    SshAuthMethod::Password { .. } => {
                        let password = field(&self.form_password);
                        SshAuthMethod::Password { password }
                    }
                    SshAuthMethod::PrivateKey { .. } => SshAuthMethod::PrivateKey {
                        path: field(&self.form_private_key_path),
                        passphrase: field(&self.form_private_key_passphrase),
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
            advanced,
            note,
        };

        let result = if let Some(id) = editing_id {
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

    fn rebuild_view_model(&mut self, cx: &mut Context<Self>) {
        self.load_profiles_cache();
        let sessions = self.service.session_summaries();
        self.vm = SshViewModel {
            profiles: Self::build_profiles(
                &self.profiles_cache,
                self.selected_profile_id,
            ),
            sessions: Self::build_sessions(&sessions, self.selected_session_id),
            file_tree: Self::build_file_tree(
                self.selected_session_id.as_ref(),
                &self.service,
                self.selected_file_path.as_deref(),
            ),
            terminal: Self::build_terminal(self.selected_session_id.as_ref(), &self.service),
            transfers: Self::build_transfers(self.selected_session_id.as_ref(), &self.service),
        };
        self.sync_terminal_pane(cx);
    }

    // ===== Build Functions =====

    fn build_profiles(
        profiles: &[crate::model::Profile],
        selected_id: Option<i64>,
    ) -> Vec<ProfileItem> {
        profiles
            .iter()
            .map(|p| ProfileItem {
                id: p.id,
                name: p.name.clone(),
                endpoint: format!("{}:{}", p.host, p.port),
                protocol_badge: p.protocol.display().to_string(),
                is_selected: selected_id == Some(p.id),
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
                    SessionStatus::Disconnected => hsla(0.08, 0.8, 0.5, 1.0),
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

    fn build_file_tree(
        session_id: Option<&SessionId>,
        service: &SshService,
        selected_file_path: Option<&str>,
    ) -> FileTreeViewModel {
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
                let is_selected = selected_file_path == Some(path.as_str());
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
                    is_selected,
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

    pub(crate) fn build_terminal(session_id: Option<&SessionId>, service: &SshService) -> TerminalViewModel {
        let Some(id) = session_id else {
            return Self::empty_terminal_vm("未连接", TerminalKind::Shell);
        };
        if let Some(frame) = service.terminal_snapshot(id) {
            return TerminalViewModel {
                status: frame.status_text,
                lines: frame.lines,
                grid: frame.grid,
                cols: frame.cols,
                rows: frame.rows,
                display_offset: frame.display_offset,
                max_display_offset: frame.max_display_offset,
                cursor_visible: frame.cursor_visible,
                cursor_row: frame.cursor_row,
                cursor_col: frame.cursor_col,
                modes: frame.modes,
                terminal_kind: frame.terminal_kind,
            };
        }
        if let Some(summary) = service.session_summary(id) {
            return Self::empty_terminal_vm(&summary.message, summary.terminal_kind);
        }
        Self::empty_terminal_vm("未连接", TerminalKind::Shell)
    }

    fn empty_terminal_vm(status: &str, kind: TerminalKind) -> TerminalViewModel {
        TerminalViewModel {
            status: status.to_string(),
            lines: Vec::new(),
            grid: Vec::new(),
            cols: 0,
            rows: 0,
            display_offset: 0,
            max_display_offset: 0,
            cursor_visible: false,
            cursor_row: 0,
            cursor_col: 0,
            modes: TerminalModes::default(),
            terminal_kind: kind,
        }
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
                grid: Vec::new(),
                cols: 0,
                rows: 0,
                display_offset: 0,
                max_display_offset: 0,
                cursor_visible: false,
                cursor_row: 0,
                cursor_col: 0,
                modes: TerminalModes::default(),
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
                        self.profile_list_scroll.clone(),
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
                            .child(session_tabs::render_session_tabs(
                                &self.vm.sessions,
                                &self.session_tab_scroll,
                                cx,
                            ))
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
                                                self.selected_session_id,
                                                self.file_list_scroll.clone(),
                                                self.file_path_input.clone(),
                                                self.follow_terminal,
                                                matches!(
                                                    self.vm.terminal.terminal_kind,
                                                    TerminalKind::Shell
                                                ),
                                                cx,
                                            ))
                                            .child(self.terminal_pane.clone()),
                                    )
                                    .child(transfer_panel::render_transfer_panel(
                                        &self.vm.transfers,
                                        self.transfer_panel_expanded,
                                        self.transfer_list_scroll.clone(),
                                        cx,
                                    )),
                            ),
                    ),
            )
            .when(self.show_upload_overwrite_confirm, {
                let confirm_handle = handle.clone();
                let (count, sample) = self
                    .pending_upload_batch
                    .as_ref()
                    .map(|batch| {
                        let count = batch.conflict_remotes.len();
                        let sample = batch
                            .conflict_remotes
                            .iter()
                            .next()
                            .and_then(|path| path.rsplit('/').next())
                            .unwrap_or("文件")
                            .to_string();
                        (count, sample)
                    })
                    .unwrap_or((0, String::from("文件")));
                move |root| {
                    root.child(file_upload_overwrite::render_upload_overwrite_overlay(
                        confirm_handle.clone(),
                        count,
                        &sample,
                    ))
                }
            })
            .when(self.show_file_upload_confirm, {
                let confirm_handle = handle.clone();
                let file_name = self
                    .pending_external_edit
                    .as_ref()
                    .map(|pending| pending.file_name.clone())
                    .unwrap_or_else(|| "文件".into());
                move |root| {
                    root.child(file_edit_confirm::render_file_edit_confirm_overlay(
                        confirm_handle.clone(),
                        &file_name,
                    ))
                }
            })
            .when(self.show_file_rename, {
                let rename_handle = handle.clone();
                let rename_input = self.file_rename_input.clone();
                move |root| {
                    root.child(file_rename::render_file_rename_overlay(
                        rename_handle.clone(),
                        rename_input.clone(),
                    ))
                }
            })
    }
}

fn expand_user_path(path: &str) -> std::path::PathBuf {
    let trimmed = path.trim();
    if trimmed == "~" {
        return std::env::var("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from(trimmed));
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
    }
    std::path::PathBuf::from(trimmed)
}

impl Focusable for SshView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

//! SshView 主视图 + ViewModel + 布局

mod sidebar;
mod session_tabs;
mod file_tree;
mod terminal_pane;
mod transfer_panel;
mod settings_dialog;

use std::sync::Arc;

use gpui::*;
use gpui::prelude::FluentBuilder;
use qingqi_ui::text_input::TextInput;
use crate::model::{SessionId, SessionStatus, TerminalKind};
use crate::service::{SshEvent, SshService};
use crate::terminal::TerminalLine;
use crate::transfer;

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
    pub is_dir: bool,
    pub is_selected: bool,
}

#[derive(Clone, Debug)]
pub struct TerminalViewModel {
    pub status: String,
    pub lines: Vec<TerminalLine>,
    pub cursor_visible: bool,
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
    show_settings: bool,

    // 设置表单输入框
    form_name: Entity<TextInput>,
    form_host: Entity<TextInput>,
    form_port: Entity<TextInput>,
    form_username: Entity<TextInput>,
    form_password: Entity<TextInput>,
    form_remote_root: Entity<TextInput>,
    form_local_root: Entity<TextInput>,

    event_task: Option<Task<()>>,
    last_revision: u64,
    generation: u64,
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
            show_settings: false,
            form_name: cx.new(|cx| TextInput::new(cx, "名称", "")),
            form_host: cx.new(|cx| TextInput::new(cx, "主机地址", "")),
            form_port: cx.new(|cx| TextInput::new(cx, "端口", "22")),
            form_username: cx.new(|cx| TextInput::new(cx, "用户名", "root")),
            form_password: cx.new(|cx| TextInput::new(cx, "密码", "")),
            form_remote_root: cx.new(|cx| TextInput::new(cx, "远程目录", "~")),
            form_local_root: cx.new(|cx| TextInput::new(cx, "本地目录", "~/Downloads")),
            event_task: None,
            last_revision: 0,
            generation: 0,
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

    fn on_service_event(&mut self, _event: &SshEvent, cx: &mut Context<Self>) {
        self.rebuild_view_model();
        cx.notify();
    }

    // ===== 用户交互（待 GPUI on_click 连线） =====
    #[allow(dead_code)]
    fn connect_profile(&mut self, profile_id: i64, cx: &mut Context<Self>) {
        self.selected_profile_id = Some(profile_id);
        match self.service.open_session(profile_id) {
            Ok(session_id) => {
                self.selected_session_id = Some(session_id);
            }
            Err(e) => {
                tracing::error!("连接失败: {e}");
            }
        }
        self.rebuild_view_model();
        cx.notify();
    }

    fn select_session(&mut self, session_id: SessionId, cx: &mut Context<Self>) {
        self.selected_session_id = Some(session_id);
        self.rebuild_view_model();
        cx.notify();
    }

    fn close_session(&mut self, session_id: SessionId, cx: &mut Context<Self>) {
        if self.selected_session_id == Some(session_id) {
            self.selected_session_id = None;
        }
        let _ = self.service.close_session(&session_id);
        self.rebuild_view_model();
        cx.notify();
    }

    fn toggle_transfer_panel(&mut self, cx: &mut Context<Self>) {
        self.transfer_panel_expanded = !self.transfer_panel_expanded;
        cx.notify();
    }

    fn toggle_settings(&mut self, cx: &mut Context<Self>) {
        self.show_settings = !self.show_settings;
        cx.notify();
    }

    #[allow(dead_code)]
    fn create_profile_from_form(&mut self, cx: &mut Context<Self>) {
        let name = self.form_name.read(cx).text();
        let host = self.form_host.read(cx).text();
        let port: u16 = self.form_port.read(cx).text().parse().unwrap_or(22);
        let _username = self.form_username.read(cx).text(); // SSH 连接时使用，暂存
        let password = self.form_password.read(cx).text();
        let remote_root = self.form_remote_root.read(cx).text();
        let local_root = self.form_local_root.read(cx).text();

        if name.is_empty() || host.is_empty() {
            return;
        }
        let auth = if password.is_empty() {
            crate::model::AuthConfig::Ssh { method: crate::model::SshAuthMethod::Agent }
        } else {
            crate::model::AuthConfig::Ssh { method: crate::model::SshAuthMethod::Password { password } }
        };
        let draft = crate::model::ProfileDraft {
            name,
            host,
            port,
            auth,
            paths: crate::model::PathConfig { remote_root, local_root },
            ..Default::default()
        };
        match self.service.create_profile(draft) {
            Ok(_) => {
                self.show_settings = false;
                self.rebuild_view_model();
                cx.notify();
            }
            Err(e) => {
                tracing::error!("创建 Profile 失败: {e}");
            }
        }
    }

    fn rebuild_view_model(&mut self) {
        let snap = self.service.snapshot();
        if snap.revision == self.last_revision {
            return;
        }
        self.last_revision = snap.revision;

        self.vm = SshViewModel {
            profiles: Self::build_profiles(&snap.profiles, &snap.sessions, self.selected_profile_id),
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
                let is_connected = sessions.iter().any(|s| {
                    s.profile_id == p.id && matches!(s.status, SessionStatus::Connected)
                });
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

    fn build_file_tree(
        session_id: Option<&SessionId>,
        service: &SshService,
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

        FileTreeViewModel {
            current_path,
            parent_path: parent,
            entries: entries
                .into_iter()
                .map(|e| FileEntryRow {
                    path: e.path,
                    name: if e.is_dir {
                        format!("{}/", e.name)
                    } else {
                        e.name.clone()
                    },
                    icon_name: String::new(),
                    size_text: if e.is_dir {
                        String::new()
                    } else {
                        transfer::format_size(e.size)
                    },
                    is_dir: e.is_dir,
                    is_selected: false,
                })
                .collect(),
        }
    }

    fn build_terminal(
        session_id: Option<&SessionId>,
        service: &SshService,
    ) -> TerminalViewModel {
        session_id
            .and_then(|id| service.terminal_snapshot(id))
            .map(|frame| TerminalViewModel {
                status: frame.status_text,
                lines: frame.lines,
                cursor_visible: frame.cursor_visible,
                terminal_kind: frame.terminal_kind,
            })
            .unwrap_or(TerminalViewModel {
                status: "未连接".into(),
                lines: Vec::new(),
                cursor_visible: false,
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

        let (active, completed, failed) = tasks.iter().fold((0, 0, 0), |(a, c, f), t| {
            match t.status {
                crate::model::TransferStatus::Queued | crate::model::TransferStatus::Running => {
                    (a + 1, c, f)
                }
                crate::model::TransferStatus::Completed => (a, c + 1, f),
                crate::model::TransferStatus::Failed => (a, c, f + 1),
                _ => (a, c, f),
            }
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
            .flex()
            // 左侧列
            .child(sidebar::render_sidebar(
                &self.vm.profiles,
                self.selected_profile_id,
                cx,
            ))
            // 右侧列
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
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
                                    .child(file_tree::render_file_tree(&self.vm.file_tree))
                                    .child(terminal_pane::render_terminal(&self.vm.terminal, cx)),
                            )
                            .child(transfer_panel::render_transfer_panel(
                                &self.vm.transfers,
                                self.transfer_panel_expanded,
                                cx,
                            )),
                    ),
            )
            // Overlay
            .when(self.show_settings, {
                let inputs = settings_dialog::ProfileFormInputs {
                    name: self.form_name.clone(),
                    host: self.form_host.clone(),
                    port: self.form_port.clone(),
                    username: self.form_username.clone(),
                    password: self.form_password.clone(),
                    remote_root: self.form_remote_root.clone(),
                    local_root: self.form_local_root.clone(),
                };
                move |root| {
                    root.child(settings_dialog::render_profile_editor(overlay_handle.clone(), &inputs))
                }
            })
    }
}

impl Focusable for SshView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

//! SshView 主视图 + ViewModel + 布局渲染

use std::sync::Arc;

use gpui::*;
use gpui::prelude::FluentBuilder;
use gpui_component::scroll::ScrollableElement;

use crate::model::{SessionId, SessionStatus, TerminalKind};
use crate::service::{SshEvent, SshService};
use crate::terminal::TerminalLine;
use crate::transfer;
use qingqi_ui::ui;

// ========== ViewModel ==========

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

    fn rebuild_view_model(&mut self) {
        let snap = self.service.snapshot();
        if snap.revision == self.last_revision {
            return;
        }
        self.last_revision = snap.revision;

        self.vm = SshViewModel {
            profiles: Self::build_profiles(&snap.profiles, self.selected_profile_id),
            sessions: Self::build_sessions(&snap.sessions, self.selected_session_id),
            file_tree: Self::build_file_tree(self.selected_session_id.as_ref(), &self.service),
            terminal: Self::build_terminal(self.selected_session_id.as_ref(), &self.service),
            transfers: Self::build_transfers(self.selected_session_id.as_ref(), &self.service),
        };
    }

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
                is_connected: false,
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
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .child(
                // 左侧列
                div()
                    .w(px(280.0))
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(ui::bg_surface())
                    .border_r_1()
                    .border_color(ui::border_light())
                    .child(render_sidebar_top(&self.vm.profiles))
                    .child(render_profile_list(
                        &self.vm.profiles,
                        self.selected_profile_id,
                    ))
                    .child(render_sidebar_bottom()),
            )
            .child(
                // 右侧列
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(render_session_tabs(&self.vm.sessions))
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
                                    .child(render_file_tree_pane(&self.vm.file_tree))
                                    .child(render_terminal_pane(&self.vm.terminal)),
                            )
                            .child(render_transfer_panel(
                                &self.vm.transfers,
                                self.transfer_panel_expanded,
                            )),
                    ),
            )
    }
}

impl Focusable for SshView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// ========== 子组件渲染 ==========

fn render_sidebar_top(_profiles: &[ProfileItem]) -> impl IntoElement {
    div()
        .h(px(52.0))
        .flex()
        .items_center()
        .px_3()
        .border_b_1()
        .border_color(ui::border_light())
        .child(mac_traffic_lights())
        .child(
            div()
                .ml_2()
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child("远程管理"),
        )
        .child(div().flex_1())
        .child(div().child("+"))
}

fn mac_traffic_lights() -> impl IntoElement {
    div()
        .flex()
        .gap(px(8.0))
        .px(px(4.0))
        .child(div().size(px(12.0)).rounded_full().bg(rgb(0xED6A5E)))
        .child(div().size(px(12.0)).rounded_full().bg(rgb(0xF5BF4F)))
        .child(div().size(px(12.0)).rounded_full().bg(rgb(0x61C554)))
}

fn render_profile_list(
    profiles: &[ProfileItem],
    selected_id: Option<i64>,
) -> impl IntoElement {
    div()
        .flex_1()
        .overflow_y_scrollbar()
        .p_2()
        .children(profiles.iter().map(|p| {
            render_profile_card(p, selected_id == Some(p.id))
        }))
}

fn render_profile_card(profile: &ProfileItem, is_selected: bool) -> impl IntoElement {
    div()
        .p_2()
        .mb_1()
        .rounded_md()
        .bg(if is_selected {
            hsla(0.55, 0.3, 0.5, 0.15)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .border_l_3()
        .border_color(if profile.is_connected {
            hsla(0.4, 0.8, 0.5, 1.0)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(profile.name.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(ui::text_secondary())
                        .child(profile.endpoint.clone()),
                ),
        )
}

fn render_sidebar_bottom() -> impl IntoElement {
    div()
        .h(px(48.0))
        .flex()
        .items_center()
        .justify_center()
        .border_t_1()
        .border_color(ui::border_light())
        .child("设置")
}

fn render_session_tabs(sessions: &[SessionTabItem]) -> impl IntoElement {
    div()
        .h(px(44.0))
        .flex()
        .items_center()
        .px_2()
        .bg(ui::bg_surface())
        .border_b_1()
        .border_color(ui::border_light())
        .children(sessions.iter().map(|s| {
            div()
                .px_3()
                .py_1()
                .mr_1()
                .rounded_t_md()
                .bg(if s.is_selected {
                    hsla(0.55, 0.05, 0.35, 0.5)
                } else {
                    hsla(0.0, 0.0, 0.0, 0.0)
                })
                .border_b_2()
                .border_color(if s.is_selected { s.status_color } else { hsla(0.0, 0.0, 0.0, 0.0) })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(div().size(px(8.0)).rounded_full().bg(s.status_color))
                        .child(div().text_size(px(12.0)).child(s.title.clone())),
                )
        }))
        .child(div().ml_2().child("+"))
}

fn render_file_tree_pane(tree: &FileTreeViewModel) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(ui::border_light())
        .child(
            div()
                .h(px(36.0))
                .flex()
                .items_center()
                .px_2()
                .border_b_1()
                .border_color(ui::border_light())
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(ui::text_secondary())
                        .child(tree.current_path.clone()),
                ),
        )
        .child(
            div().flex_1().overflow_y_scrollbar().children(
                tree.entries.iter().map(|e| {
                    div()
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .px_2()
                        .text_size(px(12.0))
                        .bg(if e.is_selected {
                            hsla(0.55, 0.3, 0.5, 0.15)
                        } else {
                            hsla(0.0, 0.0, 0.0, 0.0)
                        })
                        .child(e.name.clone())
                }),
            ),
        )
}

fn render_terminal_pane(term: &TerminalViewModel) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .bg(ui::bg_surface())
        .child(
            div()
                .h(px(28.0))
                .flex()
                .items_center()
                .px_2()
                .border_b_1()
                .border_color(ui::border_light())
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(ui::text_secondary())
                        .child(term.status.clone()),
                ),
        )
        .child(
            div()
                .flex_1()
                .overflow_y_scrollbar()
                .p_2()
                .font_family("Menlo")
                .children(term.lines.iter().map(|line| {
                    let mut el = div().text_size(px(12.0)).child(line.text.clone());
                    if let Some(color) = line.fg_color {
                        el = el.text_color(hsla(color[0], color[1], color[2], color[3]));
                    }
                    el
                })),
        )
}

fn render_transfer_panel(
    transfers: &TransferPanelViewModel,
    expanded: bool,
) -> impl IntoElement {
    div()
        .w_full()
        .border_t_1()
        .border_color(ui::border_light())
        .bg(ui::bg_surface())
        .child(
            div()
                .h(px(36.0))
                .flex()
                .items_center()
                .px_3()
                .justify_between()
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(ui::text_secondary())
                        .child(format!(
                            "传输记录 ({} 进行中, {} 已完成, {} 失败)",
                            transfers.active_count,
                            transfers.completed_count,
                            transfers.failed_count,
                        )),
                )
                .child(if expanded { "收起 ▲" } else { "展开 ▼" }),
        )
        .when(expanded, |root| {
            root.child(
                div().h(px(200.0)).overflow_y_scrollbar().children(
                    transfers.rows.iter().map(|row| {
                        div()
                            .h(px(32.0))
                            .flex()
                            .items_center()
                            .px_3()
                            .text_size(px(12.0))
                            .child(div().mr_2().child(row.direction_icon))
                            .child(div().flex_1().child(row.file_name.clone()))
                            .child(
                                div()
                                    .mr_2()
                                    .text_color(row.status_color)
                                    .child(row.status_text.clone()),
                            )
                    }),
                ),
            )
        })
}

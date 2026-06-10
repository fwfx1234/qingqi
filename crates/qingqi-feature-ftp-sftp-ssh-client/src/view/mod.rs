use std::{
    ops::Range,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use gpui::{
    AnyElement, App, AppContext, Context, Entity, ExternalPaths, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Render, ScrollDelta, ScrollWheelEvent, StatefulInteractiveElement,
    Styled, Task, Window, div, hsla, prelude::FluentBuilder, px, rgb, uniform_list,
};
use gpui_component::input::{Input, InputState};
use gpui_component::{
    Icon, IconName, Selectable, Sizable, Size as ComponentSize,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
    tab::{Tab, TabBar},
};
use qingqi_plugin::plugin_spec::PluginAccent;
use qingqi_ui::{theme, theme_mode, ui};

use crate::{
    model::{
        AuthMethod, Profile, ProfileDraft, RemoteProtocol, SessionId, SessionSummary,
        SshHostKeyPolicy, TlsVerifyPolicy,
    },
    protocols::RemoteEntry,
    runtime::{EditableFile, RemoteRuntime, RemoteRuntimeEvent, SessionSnapshot},
    terminal::{
        TerminalCellStyle, TerminalInput, TerminalMouseButton, TerminalMouseEventKind,
        TerminalMouseModifiers, TerminalMouseScrollDirection,
    },
    transfer::{TransferDirection, TransferSnapshot, TransferStatus},
};

mod glass;
mod mac_ui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RemoteActionKind {
    CreateDirectory,
    Rename,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProfileEditorMode {
    Create,
    Edit(i64),
}

impl ProfileEditorMode {
    fn existing_profile_id(self) -> Option<i64> {
        match self {
            Self::Create => None,
            Self::Edit(id) => Some(id),
        }
    }
}

struct ProfileEditorState {
    mode: ProfileEditorMode,
    draft: ProfileDraft,
    name_input: Entity<InputState>,
    host_input: Entity<InputState>,
    port_input: Entity<InputState>,
    username_input: Entity<InputState>,
    password_input: Entity<InputState>,
    private_key_path_input: Entity<InputState>,
    private_key_passphrase_input: Entity<InputState>,
    remote_root_input: Entity<InputState>,
    local_root_input: Entity<InputState>,
    connect_timeout_input: Entity<InputState>,
    transfer_concurrency_input: Entity<InputState>,
    pinned_host_key_input: Entity<InputState>,
    pinned_tls_sha256_input: Entity<InputState>,
    notes_input: Entity<InputState>,
}

struct RemoteActionState {
    kind: RemoteActionKind,
    target_path: Option<String>,
    target_name: String,
    input: Option<Entity<InputState>>,
    is_dir: bool,
}

#[derive(Clone, Debug)]
struct RemoteMenuState {
    x: f32,
    y: f32,
    target_path: Option<String>,
}

struct ProfileMenuState {
    x: f32,
    y: f32,
    profile_id: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct TerminalLayoutSnapshot {
    origin_x: f32,
    origin_y: f32,
    width: f32,
    height: f32,
}

pub struct FtpSftpSshView {
    runtime: Arc<RemoteRuntime>,
    focus_handle: FocusHandle,
    event_task: Option<Task<()>>,
    terminal_layout: Arc<Mutex<Option<TerminalLayoutSnapshot>>>,
    profiles: Vec<Profile>,
    sessions: Vec<SessionSummary>,
    transfers: Vec<TransferSnapshot>,
    selected_profile_id: Option<i64>,
    selected_session_id: Option<SessionId>,
    selected_remote_path: Option<String>,
    profile_editor: Option<ProfileEditorState>,
    remote_action: Option<RemoteActionState>,
    remote_menu: Option<RemoteMenuState>,
    profile_menu: Option<ProfileMenuState>,
    notice: String,
    terminal_frame: Option<crate::terminal::TerminalFrame>,
    last_terminal_revision: u64,
    /// overlay 关闭后标记需要恢复终端焦点，在 Render 中统一处理
    needs_terminal_focus: bool,
    /// 传输面板展开状态
    transfer_panel_expanded: bool,
    /// 防止系统文件对话框并发打开
    file_dialog_task: Option<Task<()>>,
}

impl FtpSftpSshView {
    pub fn new(runtime: Arc<RemoteRuntime>, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            runtime,
            focus_handle: cx.focus_handle(),
            event_task: None,
            terminal_layout: Arc::new(Mutex::new(None)),
            profiles: Vec::new(),
            sessions: Vec::new(),
            transfers: Vec::new(),
            selected_profile_id: None,
            selected_session_id: None,
            selected_remote_path: None,
            profile_editor: None,
            remote_action: None,
            remote_menu: None,
            profile_menu: None,
            notice: String::from("选择左侧连接配置，双击或回车打开连接"),
            terminal_frame: None,
            last_terminal_revision: 0,
            needs_terminal_focus: false,
            transfer_panel_expanded: false,
            file_dialog_task: None,
        };
        view.start_event_task(cx);
        view.reload();
        view
    }

    fn start_event_task(&mut self, cx: &mut Context<Self>) {
        if self.event_task.is_some() {
            return;
        }

        let runtime = Arc::clone(&self.runtime);
        self.event_task = Some(cx.spawn(async move |view, async_cx| {
            let mut events_rx = runtime.subscribe_events();
            while let Some(event) = events_rx.recv().await {
                let Ok(()) = view.update(async_cx, |view, cx| {
                    view.apply_runtime_event(&event, cx);
                }) else {
                    break;
                };
            }
        }));
    }

    fn apply_runtime_event(&mut self, event: &RemoteRuntimeEvent, cx: &mut Context<Self>) {
        let selected = self.selected_session_id.clone();
        match event {
            RemoteRuntimeEvent::ProfilesChanged => {
                self.reload();
            }
            RemoteRuntimeEvent::SessionsChanged => {
                self.refresh_session_list();
            }
            RemoteRuntimeEvent::SessionChanged(id) => {
                self.refresh_session_list();
                if selected.as_ref() == Some(id) {
                    let _ = self.refresh_selected_terminal();
                }
                if let Some(snapshot) = self.runtime.session_snapshot(id) {
                    match snapshot.summary.status {
                        crate::model::SessionStatus::Connected => {
                            self.selected_session_id = Some(id.clone());
                            self.selected_remote_path = None;
                            self.needs_terminal_focus = true;
                            let _ = self.refresh_selected_terminal();
                        }
                        crate::model::SessionStatus::Degraded => {
                            self.notice = format!(
                                "{} 部分可用: {}",
                                snapshot.summary.title, snapshot.summary.message
                            );
                        }
                        crate::model::SessionStatus::Failed => {
                            self.notice = format!(
                                "{} 连接失败: {}",
                                snapshot.summary.title, snapshot.summary.message
                            );
                        }
                        _ => {}
                    }
                }
            }
            RemoteRuntimeEvent::TransfersChanged => {
                self.transfers = self.runtime.all_transfer_snapshots();
            }
            RemoteRuntimeEvent::TerminalChanged(id) => {
                if selected.as_ref() == Some(id) {
                    let _ = self.refresh_selected_terminal();
                }
            }
            RemoteRuntimeEvent::SessionOpenFailed { error, .. } => {
                self.notice = format!("连接失败: {error}");
            }
            RemoteRuntimeEvent::ConnectionProgress { message, .. } => {
                self.notice = message.clone();
            }
            RemoteRuntimeEvent::EditReady { local_path, .. } => {
                match qingqi_platform::shell::open_path(&PathBuf::from(local_path.clone())) {
                    Ok(()) => self.notice = format!("已打开本地编辑副本 {}", local_path),
                    Err(error) => self.notice = format!("下载完成，但打开本地文件失败: {error}"),
                }
            }
        }
        cx.notify();
    }

    fn reload(&mut self) {
        self.profiles = self.runtime.list_profiles().unwrap_or_default();
        self.refresh_runtime_state();

        if self.selected_profile_id.is_none() {
            self.selected_profile_id = self.profiles.first().map(|profile| profile.id);
        }
        if let Some(editor) = self.profile_editor.as_ref()
            && let Some(profile_id) = editor.mode.existing_profile_id()
            && self.profiles.iter().all(|profile| profile.id != profile_id)
        {
            self.profile_editor = None;
        }
    }

    fn refresh_runtime_state(&mut self) {
        self.refresh_session_list();
        self.transfers = self.runtime.all_transfer_snapshots();
    }

    fn refresh_session_list(&mut self) {
        self.sessions = self.runtime.session_summaries();

        if self.selected_session_id.is_none() {
            self.selected_session_id = self
                .sessions
                .first()
                .map(|session| session.session_id.clone());
        }
        if let Some(selected) = self.selected_session_id.clone()
            && self
                .sessions
                .iter()
                .all(|session| session.session_id != selected)
        {
            self.selected_session_id = self
                .sessions
                .first()
                .map(|session| session.session_id.clone());
            self.selected_remote_path = None;
        }

        let selected_paths = self
            .selected_session()
            .map(|snapshot| {
                snapshot
                    .remote_entries
                    .iter()
                    .map(|entry| entry.path.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if let Some(selected_remote_path) = self.selected_remote_path.clone()
            && !selected_paths
                .iter()
                .any(|path| path == &selected_remote_path)
        {
            self.selected_remote_path = selected_paths.first().cloned();
        }

        if self.selected_remote_path.is_none() {
            self.selected_remote_path = selected_paths.first().cloned();
        }
    }

    /// 仅当选中 session 的终端帧版本号变化时才更新缓存的 frame，返回是否发生变化。
    /// 这样无变化的 TerminalChanged 事件不会触发重绘。
    fn refresh_selected_terminal(&mut self) -> bool {
        let Some(session_id) = self.selected_session_id.clone() else {
            return self.clear_terminal_frame();
        };
        let Some(revision) = self.runtime.terminal_revision(&session_id) else {
            return self.clear_terminal_frame();
        };
        if revision == self.last_terminal_revision && self.terminal_frame.is_some() {
            return false;
        }
        if let Some(frame) = self.runtime.terminal_frame(&session_id) {
            self.last_terminal_revision = frame.revision;
            self.terminal_frame = Some(frame);
            true
        } else {
            self.clear_terminal_frame()
        }
    }

    fn clear_terminal_frame(&mut self) -> bool {
        self.last_terminal_revision = 0;
        self.terminal_frame.take().is_some()
    }

    fn selected_profile(&self) -> Option<Profile> {
        let profile_id = self.selected_profile_id?;
        self.profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned()
    }

    fn selected_session(&self) -> Option<SessionSnapshot> {
        let session_id = self.selected_session_id.as_ref()?;
        self.runtime.session_snapshot(session_id)
    }

    fn selected_terminal_session_id(&self) -> Option<SessionId> {
        self.selected_session()
            .filter(|snapshot| snapshot.summary.has_terminal)
            .map(|snapshot| snapshot.summary.session_id)
    }

    fn selected_remote_entry(&self, snapshot: &SessionSnapshot) -> Option<RemoteEntry> {
        let selected_path = self.selected_remote_path.as_ref()?;
        snapshot
            .remote_entries
            .iter()
            .find(|entry| &entry.path == selected_path)
            .cloned()
    }

    fn selected_editable_file<'a>(
        &self,
        snapshot: &'a SessionSnapshot,
    ) -> Option<&'a EditableFile> {
        let selected_path = self.selected_remote_path.as_ref()?;
        snapshot
            .editable_files
            .iter()
            .find(|item| &item.remote_path == selected_path)
    }

    fn editable_for_path<'a>(
        &self,
        snapshot: &'a SessionSnapshot,
        remote_path: &str,
    ) -> Option<&'a EditableFile> {
        snapshot
            .editable_files
            .iter()
            .find(|item| item.remote_path == remote_path)
    }

    fn terminal_layout_snapshot(&self) -> Option<TerminalLayoutSnapshot> {
        self.terminal_layout.lock().ok().and_then(|layout| *layout)
    }

    fn select_profile(&mut self, profile_id: i64) {
        self.selected_profile_id = Some(profile_id);
        self.notice = String::from("已选择连接配置");
    }

    fn open_profile_context_menu(
        &mut self,
        x: f32,
        y: f32,
        profile_id: i64,
        _cx: &mut Context<Self>,
    ) {
        self.profile_menu = Some(ProfileMenuState { x, y, profile_id });
    }

    fn close_profile_menu(&mut self) {
        self.profile_menu = None;
        self.needs_terminal_focus = true;
    }

    fn select_session(&mut self, session_id: SessionId) {
        self.selected_session_id = Some(session_id);
        self.selected_remote_path = None;
        self.notice = String::from("已切换 session");
        self.last_terminal_revision = 0;
        self.terminal_frame = None;
        self.reload();
        self.refresh_selected_terminal();
        self.needs_terminal_focus = true;
    }

    fn select_remote_entry(&mut self, remote_path: String) {
        self.selected_remote_path = Some(remote_path);
        self.notice = String::from("已选择远端项目");
    }

    fn open_profile_creator(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.profile_editor = Some(build_profile_editor_state(
            ProfileEditorMode::Create,
            ProfileDraft::default(),
            window,
            cx,
        ));
        self.notice = String::from("新建连接配置");
    }

    fn open_profile_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile() else {
            self.notice = String::from("先选择一个连接配置");
            return;
        };
        self.profile_editor = Some(build_profile_editor_state(
            ProfileEditorMode::Edit(profile.id),
            ProfileDraft::from_profile(&profile),
            window,
            cx,
        ));
        self.notice = format!("编辑 {}", profile.name);
    }

    fn close_profile_editor(&mut self) {
        self.profile_editor = None;
        self.notice = String::from("已关闭连接配置编辑器");
        // 标记需要恢复终端焦点（在 Render 中统一处理，
        // 因为 overlay backdrop 回调中没有 Window 句柄）
        self.needs_terminal_focus = true;
    }

    fn editor_mut(&mut self) -> Option<&mut ProfileEditorState> {
        self.profile_editor.as_mut()
    }

    fn open_selected_profile(&mut self) {
        let Some(profile_id) = self.selected_profile_id else {
            self.notice = String::from("先选择一个连接配置");
            return;
        };
        if let Some(existing_session) = self
            .sessions
            .iter()
            .find(|session| session.profile_id == profile_id)
            .cloned()
        {
            self.select_session(existing_session.session_id);
            self.notice = format!("已切换到已有 session: {}", existing_session.title);
            return;
        }
        match self.runtime.open_session(profile_id) {
            Ok(session_id) => {
                self.selected_session_id = Some(session_id);
                self.selected_remote_path = None;
                self.notice = String::from("已打开新的 session");
                self.needs_terminal_focus = true;
            }
            Err(error) => {
                self.notice = format!("打开 session 失败: {error}");
            }
        }
        self.reload();
    }

    fn close_session(&mut self, session_id: SessionId) {
        match self.runtime.close_session(&session_id) {
            Ok(true) => self.notice = String::from("session 已关闭"),
            Ok(false) => self.notice = String::from("session 不存在"),
            Err(error) => self.notice = format!("关闭 session 失败: {error}"),
        }
        self.reload();
    }

    fn cancel_transfer(&mut self, transfer_id: crate::model::TransferId) {
        self.runtime.cancel_transfer(&transfer_id);
        self.notice = String::from("已取消传输任务");
        self.reload();
    }

    fn refresh_remote_entries(&mut self) {
        let Some(session_id) = self.selected_session_id.clone() else {
            self.notice = String::from("先打开一个 session");
            return;
        };
        match self.runtime.refresh_session_directory(&session_id, None) {
            Ok(_) => self.notice = String::from("已刷新远端目录"),
            Err(error) => self.notice = format!("刷新目录失败: {error}"),
        }
        self.reload();
    }

    fn go_to_parent_directory(&mut self) {
        let Some(session_id) = self.selected_session_id.clone() else {
            self.notice = String::from("先打开一个 session");
            return;
        };
        match self.runtime.parent_directory(&session_id) {
            Ok(()) => {
                self.selected_remote_path = None;
                self.notice = String::from("已返回上级目录");
            }
            Err(error) => self.notice = format!("切换上级目录失败: {error}"),
        }
        self.reload();
    }

    fn open_remote_entry_by_path(&mut self, remote_path: &str) {
        let Some(snapshot) = self.selected_session() else {
            self.notice = String::from("先打开一个 session");
            return;
        };
        let Some(entry) = snapshot
            .remote_entries
            .iter()
            .find(|entry| entry.path == remote_path)
            .cloned()
        else {
            self.notice = String::from("远端项目不存在");
            return;
        };
        self.selected_remote_path = Some(entry.path.clone());
        if entry.is_dir {
            match self
                .runtime
                .enter_directory(&snapshot.summary.session_id, &entry.path)
            {
                Ok(()) => {
                    self.selected_remote_path = None;
                    self.notice = format!("已进入 {}", entry.path);
                }
                Err(error) => self.notice = format!("进入目录失败: {error}"),
            }
        } else {
            self.edit_selected_entry();
            return;
        }
        self.reload();
    }

    fn upload_into_current_directory(&mut self, cx: &mut Context<Self>) {
        if self.file_dialog_task.is_some() {
            self.notice = String::from("已有文件选择器正在打开");
            return;
        }

        let start_directory = self
            .selected_session()
            .and_then(|snapshot| dialog_directory_from_text(&snapshot.local_root));
        self.notice = String::from("正在打开文件选择器");
        self.file_dialog_task = Some(cx.spawn(async move |view, async_cx| {
            let mut dialog = rfd::AsyncFileDialog::new().set_title("选择要上传的文件");
            if let Some(directory) = start_directory {
                dialog = dialog.set_directory(directory);
            }
            let picked = dialog.pick_files().await;
            let _ = view.update(async_cx, |view, cx| {
                view.file_dialog_task = None;
                match picked {
                    Some(files) => {
                        let local_paths = files
                            .into_iter()
                            .map(|file| file.path().to_path_buf())
                            .collect::<Vec<_>>();
                        view.upload_paths_into_current_directory(local_paths);
                    }
                    None => {
                        view.notice = String::from("已取消上传");
                    }
                }
                cx.notify();
            });
        }));
    }

    fn upload_paths_into_current_directory(&mut self, local_paths: Vec<PathBuf>) {
        let Some(snapshot) = self.selected_session() else {
            self.notice = String::from("先打开一个 session");
            return;
        };

        let mut queued = 0usize;
        let mut skipped = 0usize;
        let mut failed = Vec::new();
        let mut last_remote_path = None;

        for local_path in local_paths {
            if !local_path.is_file() {
                skipped += 1;
                continue;
            }
            let Some(file_name) = local_path.file_name().and_then(|name| name.to_str()) else {
                skipped += 1;
                continue;
            };
            let remote_path = join_remote_path(&snapshot.remote_root, file_name);
            match self
                .runtime
                .upload_file(&snapshot.summary.session_id, &local_path, &remote_path)
            {
                Ok(()) => {
                    queued += 1;
                    last_remote_path = Some(remote_path);
                }
                Err(error) => failed.push(format!("{}: {error}", local_path.display())),
            }
        }

        if let Some(path) = last_remote_path {
            self.selected_remote_path = Some(path);
        }
        self.notice = match (queued, skipped, failed.len()) {
            (0, 0, 0) => String::from("没有可上传的文件"),
            (0, skipped, 0) => format!("未上传；已跳过 {skipped} 个非文件项目"),
            (queued, 0, 0) => format!("已加入上传队列 {queued} 个文件"),
            (queued, skipped, 0) => {
                format!("已加入上传队列 {queued} 个文件，跳过 {skipped} 个项目")
            }
            (queued, skipped, failed) => {
                format!("已加入上传队列 {queued} 个文件，跳过 {skipped} 个项目，失败 {failed} 个")
            }
        };
        self.reload();
    }

    fn download_selected_entry(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = self.selected_session() else {
            self.notice = String::from("先打开一个 session");
            return;
        };
        let Some(entry) = self.selected_remote_entry(&snapshot) else {
            self.notice = String::from("先选择一个远端文件");
            return;
        };
        self.request_download_entry(snapshot, entry, cx);
    }

    fn request_download_entry(
        &mut self,
        snapshot: SessionSnapshot,
        entry: RemoteEntry,
        cx: &mut Context<Self>,
    ) {
        if entry.is_dir {
            self.notice = String::from("目录暂不支持整体下载");
            return;
        }
        if self.file_dialog_task.is_some() {
            self.notice = String::from("已有文件选择器正在打开");
            return;
        }
        let default_target = match self
            .runtime
            .default_download_path(&snapshot.summary.session_id, &entry.path)
        {
            Ok(path) => path,
            Err(error) => {
                self.notice = format!("准备下载路径失败: {error}");
                return;
            }
        };
        let start_directory = default_target.parent().map(|path| path.to_path_buf());
        let default_file_name = default_target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("downloaded-file")
            .to_string();
        self.notice = String::from("正在打开保存对话框");
        self.file_dialog_task = Some(cx.spawn(async move |view, async_cx| {
            let mut dialog = rfd::AsyncFileDialog::new()
                .set_title("保存远端文件")
                .set_file_name(default_file_name);
            if let Some(directory) = start_directory {
                dialog = dialog.set_directory(directory);
            }
            let Some(save_target) = dialog
                .save_file()
                .await
                .map(|file| file.path().to_path_buf())
            else {
                let _ = view.update(async_cx, |view, cx| {
                    view.file_dialog_task = None;
                    view.notice = String::from("已取消下载");
                    cx.notify();
                });
                return;
            };
            let _ = view.update(async_cx, |view, cx| {
                view.file_dialog_task = None;
                match view.runtime.download_entry(
                    &snapshot.summary.session_id,
                    &entry.path,
                    &save_target,
                ) {
                    Ok(local_path) => {
                        view.notice = format!("已下载到 {}", local_path.display());
                        view.reload();
                    }
                    Err(error) => {
                        view.notice = format!("下载失败: {error}");
                    }
                }
                cx.notify();
            });
        }));
    }

    fn edit_selected_entry(&mut self) {
        let Some(snapshot) = self.selected_session() else {
            self.notice = String::from("先打开一个 session");
            return;
        };
        let Some(entry) = self.selected_remote_entry(&snapshot) else {
            self.notice = String::from("先选择一个远端文件");
            return;
        };
        self.edit_entry(&snapshot, &entry);
    }

    fn edit_entry(&mut self, snapshot: &SessionSnapshot, entry: &RemoteEntry) {
        if entry.is_dir {
            self.notice = String::from("目录不能直接编辑");
            return;
        }
        match self
            .runtime
            .download_for_edit(&snapshot.summary.session_id, &entry.path)
        {
            Ok(local_path) => match qingqi_platform::shell::open_path(&local_path) {
                Ok(()) => {
                    self.notice = format!("已打开本地编辑副本 {}", local_path.display());
                }
                Err(error) => {
                    self.notice = format!("下载完成，但打开本地文件失败: {error}");
                }
            },
            Err(error) => self.notice = format!("准备编辑失败: {error}"),
        }
        self.reload();
    }

    fn upload_back_for_path(&mut self, remote_path: &str) {
        let Some(snapshot) = self.selected_session() else {
            self.notice = String::from("先打开一个 session");
            return;
        };
        let Some(editable) = self.editable_for_path(&snapshot, remote_path) else {
            self.notice = String::from("当前文件还没有本地编辑副本");
            return;
        };
        match self.runtime.upload_edited_file(
            &snapshot.summary.session_id,
            &editable.local_path,
            &editable.remote_path,
        ) {
            Ok(()) => self.notice = format!("已回传 {}", editable.remote_path),
            Err(error) => self.notice = format!("回传失败: {error}"),
        }
        self.reload();
    }

    fn open_create_directory_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.selected_session() else {
            self.notice = String::from("先打开一个 session");
            return;
        };
        self.remote_action = Some(RemoteActionState {
            kind: RemoteActionKind::CreateDirectory,
            target_path: Some(snapshot.remote_root.clone()),
            target_name: String::new(),
            input: Some(compact_input(window, cx, "", "new-folder", false)),
            is_dir: true,
        });
        self.remote_menu = None;
        self.notice = String::from("输入新目录名称");
    }

    fn open_rename_prompt_for_path(
        &mut self,
        remote_path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(snapshot) = self.selected_session() else {
            self.notice = String::from("先打开一个 session");
            return;
        };
        let Some(entry) = snapshot
            .remote_entries
            .iter()
            .find(|entry| entry.path == remote_path)
            .cloned()
        else {
            self.notice = String::from("远端项目不存在");
            return;
        };
        self.selected_remote_path = Some(entry.path.clone());
        self.remote_action = Some(RemoteActionState {
            kind: RemoteActionKind::Rename,
            target_path: Some(entry.path.clone()),
            target_name: entry.name.clone(),
            input: Some(compact_input(window, cx, &entry.name, "rename", false)),
            is_dir: entry.is_dir,
        });
        self.remote_menu = None;
        self.notice = format!("重命名 {}", entry.name);
    }

    fn open_delete_prompt_for_path(&mut self, remote_path: &str) {
        let Some(snapshot) = self.selected_session() else {
            self.notice = String::from("先打开一个 session");
            return;
        };
        let Some(entry) = snapshot
            .remote_entries
            .iter()
            .find(|entry| entry.path == remote_path)
            .cloned()
        else {
            self.notice = String::from("远端项目不存在");
            return;
        };
        self.selected_remote_path = Some(entry.path.clone());
        self.remote_action = Some(RemoteActionState {
            kind: RemoteActionKind::Delete,
            target_path: Some(entry.path.clone()),
            target_name: entry.name.clone(),
            input: None,
            is_dir: entry.is_dir,
        });
        self.remote_menu = None;
        self.notice = format!("确认删除 {}", entry.name);
    }

    fn close_remote_action(&mut self) {
        self.remote_action = None;
        self.notice = String::from("已取消远端操作");
        self.needs_terminal_focus = true;
    }

    fn confirm_remote_action(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = self.selected_session() else {
            self.notice = String::from("先打开一个 session");
            self.remote_action = None;
            return;
        };
        let Some(action) = self.remote_action.take() else {
            return;
        };

        match action.kind {
            RemoteActionKind::CreateDirectory => {
                let Some(input) = action.input.as_ref() else {
                    self.notice = String::from("缺少目录名称输入");
                    return;
                };
                let name = input.read(cx).value().trim().to_string();
                if name.is_empty() {
                    self.notice = String::from("目录名称不能为空");
                    self.remote_action = Some(action);
                    return;
                }
                let base = action
                    .target_path
                    .clone()
                    .unwrap_or_else(|| snapshot.remote_root.clone());
                let remote_path = join_remote_path(&base, &name);
                match self
                    .runtime
                    .create_remote_directory(&snapshot.summary.session_id, &remote_path)
                {
                    Ok(()) => {
                        self.selected_remote_path = Some(remote_path.clone());
                        self.notice = format!("已创建目录 {}", remote_path);
                    }
                    Err(error) => self.notice = format!("创建目录失败: {error}"),
                }
            }
            RemoteActionKind::Rename => {
                let Some(target_path) = action.target_path.as_ref() else {
                    self.notice = String::from("缺少待重命名路径");
                    return;
                };
                let Some(input) = action.input.as_ref() else {
                    self.notice = String::from("缺少新名称输入");
                    return;
                };
                let new_name = input.read(cx).value().trim().to_string();
                if new_name.is_empty() {
                    self.notice = String::from("新名称不能为空");
                    self.remote_action = Some(action);
                    return;
                }
                let parent = remote_parent_path(target_path);
                let new_path = join_remote_path(&parent, &new_name);
                match self.runtime.rename_remote_entry(
                    &snapshot.summary.session_id,
                    target_path,
                    &new_path,
                ) {
                    Ok(()) => {
                        self.selected_remote_path = Some(new_path.clone());
                        self.notice = format!("已重命名为 {}", new_path);
                    }
                    Err(error) => self.notice = format!("重命名失败: {error}"),
                }
            }
            RemoteActionKind::Delete => {
                let Some(target_path) = action.target_path.as_ref() else {
                    self.notice = String::from("缺少待删除路径");
                    return;
                };
                match self.runtime.remove_remote_entry(
                    &snapshot.summary.session_id,
                    target_path,
                    action.is_dir,
                ) {
                    Ok(()) => {
                        if self
                            .selected_remote_path
                            .as_ref()
                            .is_some_and(|selected| selected == target_path)
                        {
                            self.selected_remote_path = None;
                        }
                        self.notice = format!("已删除 {}", action.target_name);
                    }
                    Err(error) => self.notice = format!("删除失败: {error}"),
                }
            }
        }
        self.reload();
        self.needs_terminal_focus = true;
    }

    fn open_remote_menu(&mut self, x: f32, y: f32, target_path: Option<String>) {
        self.remote_menu = Some(RemoteMenuState { x, y, target_path });
    }

    fn close_remote_menu(&mut self) {
        self.remote_menu = None;
        self.needs_terminal_focus = true;
    }

    fn choose_private_key_path(&mut self, cx: &mut Context<Self>) {
        if self.file_dialog_task.is_some() {
            self.notice = String::from("已有文件选择器正在打开");
            return;
        }
        let Some(editor) = self.profile_editor.as_ref() else {
            self.notice = String::from("当前没有打开配置编辑器");
            return;
        };
        let current_path = editor.draft.auth.private_key_path.clone();
        self.notice = String::from("正在打开文件选择器");
        self.file_dialog_task = Some(cx.spawn(async move |view, async_cx| {
            let mut dialog = rfd::AsyncFileDialog::new().set_title("选择私钥文件");
            if let Some(directory) = dialog_directory_from_text(&current_path) {
                dialog = dialog.set_directory(directory);
            }
            let picked = dialog.pick_file().await;
            let _ = view.update(async_cx, |view, cx| {
                view.file_dialog_task = None;
                match picked {
                    Some(path) => {
                        let path_string = path.path().display().to_string();
                        if let Some(editor) = view.profile_editor.as_mut() {
                            editor.draft.auth.private_key_path = path_string.clone();

                            view.notice = String::from("已更新私钥路径");
                        } else {
                            view.notice = String::from("当前没有打开配置编辑器");
                        }
                    }
                    None => view.notice = String::from("已取消选择私钥"),
                }
                cx.notify();
            });
        }));
    }

    fn choose_local_root(&mut self, cx: &mut Context<Self>) {
        if self.file_dialog_task.is_some() {
            self.notice = String::from("已有文件选择器正在打开");
            return;
        }
        let Some(editor) = self.profile_editor.as_ref() else {
            self.notice = String::from("当前没有打开配置编辑器");
            return;
        };
        let current_path = editor.draft.paths.local_root.clone();
        self.notice = String::from("正在打开文件选择器");
        self.file_dialog_task = Some(cx.spawn(async move |view, async_cx| {
            let mut dialog = rfd::AsyncFileDialog::new().set_title("选择默认下载目录");
            if let Some(directory) = dialog_directory_from_text(&current_path) {
                dialog = dialog.set_directory(directory);
            }
            let picked = dialog.pick_folder().await;
            let _ = view.update(async_cx, |view, cx| {
                view.file_dialog_task = None;
                match picked {
                    Some(path) => {
                        let path_string = path.path().display().to_string();
                        if let Some(editor) = view.profile_editor.as_mut() {
                            editor.draft.paths.local_root = path_string.clone();

                            view.notice = String::from("已更新本地目录");
                        } else {
                            view.notice = String::from("当前没有打开配置编辑器");
                        }
                    }
                    None => view.notice = String::from("已取消选择本地目录"),
                }
                cx.notify();
            });
        }));
    }

    fn set_editor_protocol(
        &mut self,
        protocol: RemoteProtocol,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = self.editor_mut() else {
            return;
        };
        let previous_protocol = editor.draft.protocol;
        editor.draft.protocol = protocol;
        if editor.draft.port == 0 || editor.draft.port == previous_protocol.default_port() {
            editor.draft.port = protocol.default_port();
        }
        if editor.draft.paths.remote_root.trim().is_empty()
            || editor.draft.paths.remote_root == previous_protocol.default_remote_root()
        {
            editor.draft.paths.remote_root = protocol.default_remote_root().to_string();
        }
        editor.port_input.update(cx, |input, input_cx| {
            input.set_value(editor.draft.port.to_string(), window, input_cx);
        });
        editor.remote_root_input.update(cx, |input, input_cx| {
            input.set_value(editor.draft.paths.remote_root.clone(), window, input_cx);
            input.set_placeholder(protocol.default_remote_root(), window, input_cx);
        });
        self.notice = format!("协议已切换为 {}", protocol.label());
    }

    fn set_editor_auth_method(&mut self, method: AuthMethod) {
        if let Some(editor) = self.editor_mut() {
            editor.draft.auth.method = method;
            self.notice = format!("认证方式已切换为 {}", method.label());
        }
    }

    fn set_editor_ssh_policy(&mut self, policy: SshHostKeyPolicy) {
        if let Some(editor) = self.editor_mut() {
            editor.draft.security.ssh_host_key = policy;
            self.notice = format!("SSH 安全策略已切换为 {}", policy.label());
        }
    }

    fn set_editor_tls_policy(&mut self, policy: TlsVerifyPolicy) {
        if let Some(editor) = self.editor_mut() {
            editor.draft.security.tls_verify = policy;
            self.notice = format!("TLS 校验策略已切换为 {}", policy.label());
        }
    }

    fn toggle_editor_passive_mode(&mut self) {
        if let Some(editor) = self.editor_mut() {
            editor.draft.limits.passive_mode = !editor.draft.limits.passive_mode;
            self.notice = if editor.draft.limits.passive_mode {
                String::from("已启用 FTP 被动模式")
            } else {
                String::from("已关闭 FTP 被动模式")
            };
        }
    }

    fn save_profile_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.profile_editor.as_ref() else {
            self.notice = String::from("当前没有可保存的配置");
            return;
        };

        let mut draft = editor.draft.clone();
        draft.name = editor.name_input.read(cx).value().to_string();
        draft.host = editor.host_input.read(cx).value().to_string();
        draft.port = editor
            .port_input
            .read(cx)
            .value()
            .trim()
            .parse::<u16>()
            .unwrap_or(draft.protocol.default_port());
        draft.auth.username = editor.username_input.read(cx).value().to_string();
        draft.auth.password = editor.password_input.read(cx).value().to_string();
        draft.auth.private_key_path = editor.private_key_path_input.read(cx).value().to_string();
        draft.auth.private_key_passphrase = editor
            .private_key_passphrase_input
            .read(cx)
            .value()
            .to_string();
        draft.paths.remote_root = editor.remote_root_input.read(cx).value().to_string();
        draft.paths.local_root = editor.local_root_input.read(cx).value().to_string();
        draft.security.pinned_host_key = editor.pinned_host_key_input.read(cx).value().to_string();
        draft.security.pinned_tls_sha256 =
            editor.pinned_tls_sha256_input.read(cx).value().to_string();
        draft.limits.connect_timeout_secs = editor
            .connect_timeout_input
            .read(cx)
            .value()
            .trim()
            .parse::<u16>()
            .unwrap_or(draft.limits.connect_timeout_secs);
        draft.limits.transfer_concurrency = editor
            .transfer_concurrency_input
            .read(cx)
            .value()
            .trim()
            .parse::<u16>()
            .unwrap_or(draft.limits.transfer_concurrency);
        draft.notes = editor.notes_input.read(cx).value().to_string();

        match editor.mode {
            ProfileEditorMode::Create => match self.runtime.create_profile(draft.normalize()) {
                Ok(profile) => {
                    self.selected_profile_id = Some(profile.id);
                    self.profile_editor = None;
                    self.notice = format!("已创建连接 {}", profile.name);
                }
                Err(error) => {
                    self.notice = format!("创建连接失败: {error}");
                    return;
                }
            },
            ProfileEditorMode::Edit(profile_id) => {
                match self.runtime.update_profile(profile_id, draft.normalize()) {
                    Ok(Some(profile)) => {
                        self.selected_profile_id = Some(profile.id);
                        self.profile_editor = None;
                        self.notice = format!("已保存连接 {}", profile.name);
                    }
                    Ok(None) => {
                        self.notice = String::from("连接配置不存在，可能已被删除");
                        return;
                    }
                    Err(error) => {
                        self.notice = format!("保存连接失败: {error}");
                        return;
                    }
                }
            }
        }

        self.reload();
    }

    fn delete_selected_profile(&mut self) {
        let Some(profile_id) = self.selected_profile_id else {
            self.notice = String::from("先选择一个连接配置");
            return;
        };
        if self.runtime.has_live_session(profile_id) {
            self.notice = String::from("请先关闭这个连接的活动 session");
            return;
        }
        match self.runtime.delete_profile(profile_id) {
            Ok(true) => {
                self.notice = String::from("已删除连接配置");
                self.profile_editor = None;
                self.selected_profile_id = self
                    .profiles
                    .iter()
                    .find(|profile| profile.id != profile_id)
                    .map(|profile| profile.id);
            }
            Ok(false) => self.notice = String::from("连接配置不存在"),
            Err(error) => self.notice = format!("删除连接失败: {error}"),
        }
        self.reload();
    }

    fn focus_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle(cx));
        cx.notify();
    }

    /// 若当前选中 session 拥有可用终端，则尝试聚焦。
    /// 用于切换 session、关闭 overlay 等场景后自动恢复焦点。
    fn focus_terminal_if_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_terminal_session_id().is_some() {
            window.focus(&self.focus_handle(cx));
            cx.notify();
        }
    }

    fn handle_terminal_key(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 总是阻止事件冒泡，避免 gpui 全局快捷键层吞掉按键。
        // 无论当前 session 是否有终端，都阻止传播，
        // 否则焦点在终端时按下的键会被外层快捷键误消费。
        cx.stop_propagation();

        if event.keystroke.key == "escape" {
            if self.remote_menu.is_some() {
                self.remote_menu = None;
                cx.notify();
                return;
            }
            if self.profile_menu.is_some() {
                self.profile_menu = None;
                cx.notify();
                return;
            }
        }

        let Some(input) = terminal_input_for_event(event, cx) else {
            return;
        };

        let Some(session_id) = self.selected_terminal_session_id() else {
            return;
        };

        if let Err(error) = self.runtime.send_terminal_input(&session_id, input) {
            self.notice = format!("终端输入失败: {error}");
            cx.notify();
        }
    }

    fn handle_terminal_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_terminal(window, cx);
        let Some(session_id) = self.selected_terminal_session_id() else {
            cx.stop_propagation();
            return;
        };
        let Some(frame) = self.terminal_frame.as_ref() else {
            cx.stop_propagation();
            return;
        };
        if !frame.input.mouse_reporting {
            cx.stop_propagation();
            return;
        }
        let input = self
            .terminal_layout_snapshot()
            .and_then(|layout| terminal_mouse_press_input(event, frame, layout));
        if let Some(input) = input {
            let _ = self.runtime.send_terminal_input(&session_id, input);
        }
        cx.stop_propagation();
    }

    fn handle_terminal_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session_id) = self.selected_terminal_session_id() else {
            return;
        };
        let Some(frame) = self.terminal_frame.as_ref() else {
            return;
        };
        if !frame.input.mouse_reporting {
            return;
        }
        let input = self
            .terminal_layout_snapshot()
            .and_then(|layout| terminal_mouse_release_input(event, frame, layout));
        if let Some(input) = input {
            let _ = self.runtime.send_terminal_input(&session_id, input);
            cx.stop_propagation();
        }
    }

    fn handle_terminal_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session_id) = self.selected_terminal_session_id() else {
            return;
        };
        let Some(frame) = self.terminal_frame.as_ref() else {
            return;
        };
        if !frame.input.mouse_reporting {
            return;
        }
        let input = self
            .terminal_layout_snapshot()
            .and_then(|layout| terminal_mouse_move_input(event, frame, layout));
        if let Some(input) = input {
            let _ = self.runtime.send_terminal_input(&session_id, input);
            cx.stop_propagation();
        }
    }

    fn handle_terminal_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session_id) = self.selected_terminal_session_id() else {
            return;
        };
        let Some(frame) = self.terminal_frame.as_ref() else {
            return;
        };
        if !frame.input.mouse_reporting {
            return;
        }
        let input = self
            .terminal_layout_snapshot()
            .and_then(|layout| terminal_mouse_scroll_input(event, frame, layout));
        if let Some(input) = input {
            let _ = self.runtime.send_terminal_input(&session_id, input);
            cx.stop_propagation();
        }
    }
}

impl Render for FtpSftpSshView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // overlay 关闭后恢复终端焦点
        if self.needs_terminal_focus {
            self.needs_terminal_focus = false;
            self.focus_terminal_if_active(window, cx);
        }
        let selected_profile = self.selected_profile();
        let selected_session = self.selected_session();
        let selected_entry = selected_session
            .as_ref()
            .and_then(|snapshot| self.selected_remote_entry(snapshot));
        let editable_local_path = selected_session
            .as_ref()
            .and_then(|snapshot| self.selected_editable_file(snapshot))
            .map(|item| item.local_path.display().to_string());
        let dark = theme_mode::is_dark();
        let chrome = mac_ui::workspace_chrome_config();
        let handle = cx.entity();
        let titlebar = titlebar_slot(
            &self.sessions,
            self.selected_session_id.clone(),
            &handle,
            cx,
        )
        .into_any_element();
        let terminal_focused = self.focus_handle.is_focused(window);
        let terminal_frame = self.terminal_frame.as_ref();

        div()
            .relative()
            .size_full()
            .bg(theme::semantic().bg_glass)
            .rounded(px(12.0))
            .overflow_hidden()
            .shadow(glass::shadow())
            .font_family(ui::font_ui())
            .text_color(theme::semantic().text_primary)
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    if view.profile_editor.is_some() {
                        view.close_profile_editor();
                    }
                    if view.remote_action.is_some() {
                        view.close_remote_action();
                    }
                    if view.remote_menu.is_some() || view.profile_menu.is_some() {
                        view.remote_menu = None;
                        view.profile_menu = None;
                    }
                    cx.stop_propagation();
                } else if event.keystroke.key == "enter"
                    && view.profile_editor.is_none()
                    && view.remote_action.is_none()
                    && view.remote_menu.is_none()
                    && view.profile_menu.is_none()
                    && view.selected_terminal_session_id().is_none()
                {
                    view.open_selected_profile();
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .size_full()
                    .bg(glass::bg(dark))
                    .pt(px(chrome.metrics().content_top_padding + 6.0))
                    .pl(px(8.0))
                    .pr(px(8.0))
                    .pb(px(6.0))
                    .gap_0p5()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(0.0))
                            .flex()
                            .gap_1()
                            .child(left_sidebar(
                                &self.profiles,
                                &self.sessions,
                                self.selected_profile_id,
                                cx,
                                dark,
                            ))
                            .child(main_workspace(
                                selected_session.as_ref(),
                                terminal_frame,
                                selected_entry.as_ref(),
                                editable_local_path.as_deref(),
                                terminal_focused,
                                self.focus_handle.clone(),
                                Arc::clone(&self.runtime),
                                Arc::clone(&self.terminal_layout),
                                window,
                                cx,
                                dark,
                            )),
                    )
                    .child(transfer_strip(
                        &self.transfers,
                        self.notice.clone(),
                        self.transfer_panel_expanded,
                        cx,
                        dark,
                    )),
            )
            .when(self.profile_editor.is_some(), |root| {
                root.child(profile_editor_overlay(
                    handle.clone(),
                    self.profile_editor.as_ref().expect("profile editor"),
                    cx,
                    dark,
                ))
            })
            .when(self.remote_action.is_some(), |root| {
                root.child(remote_action_overlay(
                    handle.clone(),
                    self.remote_action.as_ref().expect("remote action"),
                    cx,
                    dark,
                ))
            })
            .when(self.remote_menu.is_some(), |root| {
                root.child(remote_menu_overlay(
                    self.remote_menu.as_ref().expect("remote menu"),
                    selected_session.as_ref(),
                    selected_profile.as_ref(),
                    selected_entry.as_ref(),
                    editable_local_path.as_deref(),
                    cx,
                    dark,
                ))
            })
            .when(self.profile_menu.is_some(), |root| {
                root.child(profile_menu_overlay(
                    self.profile_menu.as_ref().expect("profile menu"),
                    cx,
                    dark,
                ))
            })
            .child(ui::popup_window_chrome_with_titlebar_slot(
                chrome,
                Some(titlebar),
            ))
    }
}

impl Focusable for FtpSftpSshView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn titlebar_slot(
    sessions: &[SessionSummary],
    selected_session_id: Option<SessionId>,
    handle: &Entity<FtpSftpSshView>,
    _cx: &mut Context<FtpSftpSshView>,
) -> impl IntoElement {
    let h = handle.clone();
    let selected_index = sessions.iter().position(|s| {
        selected_session_id
            .as_ref()
            .is_some_and(|current| current == &s.session_id)
    });

    div()
        .flex()
        .items_center()
        .h_full()
        .child(
            div().pl(px(80.0)).flex_1().flex().items_center().child(
                TabBar::new("session-tabs")
                    .underline()
                    .selected_index(selected_index.unwrap_or(0))
                    .children(sessions.iter().enumerate().map(|(index, session)| {
                        let session_id = session.session_id.clone();
                        let close_id = session.session_id.clone();
                        let close_handle = h.clone();
                        let select_handle = h.clone();
                        Tab::new()
                            .label(&session.title)
                            .selected(
                                selected_session_id
                                    .as_ref()
                                    .is_some_and(|current| current == &session.session_id),
                            )
                            .suffix(
                                Button::new(("session-close", index))
                                    .icon(IconName::Close)
                                    .ghost()
                                    .xsmall()
                                    .on_click(move |_, _, cx| {
                                        let _ = cx.update_entity(&close_handle, |view, _cx| {
                                            view.close_session(close_id.clone());
                                        });
                                    }),
                            )
                            .on_click(move |_, _, cx| {
                                let _ = cx.update_entity(&select_handle, |view, _| {
                                    view.select_session(session_id.clone());
                                });
                            })
                    })),
            ),
        )
        .child(
            Button::new("titlebar-add-profile")
                .icon(IconName::Plus)
                .ghost()
                .xsmall()
                .on_click(move |_, window, cx| {
                    let _ = cx.update_entity(&h, |view, cx| {
                        view.open_profile_creator(window, cx);
                    });
                }),
        )
}

fn left_sidebar(
    profiles: &[Profile],
    sessions: &[SessionSummary],
    selected_profile_id: Option<i64>,
    cx: &mut Context<FtpSftpSshView>,
    dark: bool,
) -> impl IntoElement {
    glass_panel(px(272.0), dark)
        .child(
            div()
                .h(px(40.0))
                .flex()
                .items_center()
                .justify_between()
                .px_2()
                .child(
                    div()
                        .min_w(px(0.0))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(IconName::PanelLeft)
                                .with_size(ComponentSize::Small)
                                .text_color(theme::semantic().text_secondary),
                        )
                        .child(
                            div()
                                .min_w(px(0.0))
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child("连接"),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .line_clamp(1)
                                        .text_color(theme::semantic().text_secondary)
                                        .child(format!("{} 个配置", profiles.len())),
                                ),
                        ),
                )
                .child(
                    Button::new("new-profile")
                        .icon(IconName::Plus)
                        .ghost()
                        .xsmall()
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_profile_creator(window, cx);
                        })),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scrollbar()
                .flex()
                .flex_col()
                .gap_0p5()
                .px_1()
                .py_1()
                .when(profiles.is_empty(), |list| {
                    list.child(empty_state(
                        IconName::Inbox,
                        "还没有连接配置，点击右上角新建",
                    ))
                })
                .children(profiles.iter().map(|profile| {
                    let selected = selected_profile_id == Some(profile.id);
                    let profile_id = profile.id;
                    let has_session = sessions.iter().any(|s| s.profile_id == profile_id);
                    let endpoint = profile.endpoint();
                    div()
                        .id(("remote-profile-row", profile.id as usize))
                        .relative()
                        .overflow_hidden()
                        .h(px(58.0))
                        .rounded(px(8.0))
                        .bg(if selected {
                            if dark {
                                hsla(0.0, 0.0, 1.0, 0.075)
                            } else {
                                hsla(215.0 / 360.0, 0.20, 0.91, 0.82)
                            }
                        } else {
                            hsla(0.0, 0.0, 0.0, 0.0)
                        })
                        .border_1()
                        .border_color(if selected {
                            theme::rgba_with_alpha(theme::semantic().border_strong, 0.14)
                        } else {
                            hsla(0.0, 0.0, 0.0, 0.0)
                        })
                        .cursor_pointer()
                        .hover(move |style| {
                            style.bg(if selected {
                                if dark {
                                    hsla(0.0, 0.0, 1.0, 0.09)
                                } else {
                                    hsla(215.0 / 360.0, 0.20, 0.89, 0.90)
                                }
                            } else {
                                glass::hover_bg(dark)
                            })
                        })
                        .on_click(cx.listener(move |view, event: &gpui::ClickEvent, _, _| {
                            view.select_profile(profile_id);
                            if event.click_count() >= 2 {
                                view.open_selected_profile();
                            }
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |view, event: &MouseDownEvent, _, cx| {
                                view.select_profile(profile_id);
                                view.open_profile_context_menu(
                                    event.position.x.into(),
                                    event.position.y.into(),
                                    profile_id,
                                    cx,
                                );
                            }),
                        )
                        .child(
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .child(div().w(px(3.0)).h_full().flex_none().bg(if selected {
                                    theme::rgba_with_alpha(
                                        ui::accent_color(PluginAccent::Cyan),
                                        if dark { 0.70 } else { 0.62 },
                                    )
                                } else if has_session {
                                    theme::rgba_with_alpha(theme::semantic().success, 0.42)
                                } else {
                                    hsla(0.0, 0.0, 0.0, 0.0)
                                }))
                                .child(
                                    div()
                                        .min_w(px(0.0))
                                        .flex_1()
                                        .pl(px(10.0))
                                        .pr(px(6.0))
                                        .py(px(6.0))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_2()
                                        .child(
                                            div()
                                                .min_w(px(0.0))
                                                .flex_1()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_1()
                                                        .child(protocol_badge(
                                                            profile.protocol_label(),
                                                            selected,
                                                            dark,
                                                        ))
                                                        .child(
                                                            div()
                                                                .min_w(px(0.0))
                                                                .text_size(px(13.0))
                                                                .font_weight(if selected {
                                                                    gpui::FontWeight::MEDIUM
                                                                } else {
                                                                    gpui::FontWeight::NORMAL
                                                                })
                                                                .line_clamp(1)
                                                                .child(profile.name.clone()),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .min_w(px(0.0))
                                                        .text_size(px(12.0))
                                                        .line_clamp(1)
                                                        .font_family(ui::font_mono())
                                                        .text_color(
                                                            theme::semantic().text_secondary,
                                                        )
                                                        .child(endpoint),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex_none()
                                                .flex()
                                                .items_center()
                                                .gap_0p5()
                                                .child(status_pill(has_session, dark))
                                                .child(
                                                    Button::new((
                                                        "profile-connect",
                                                        profile.id as usize,
                                                    ))
                                                    .icon(IconName::ArrowRight)
                                                    .xsmall()
                                                    .ghost()
                                                    .tooltip(if has_session {
                                                        "切换到已有 session"
                                                    } else {
                                                        "连接"
                                                    })
                                                    .on_click(cx.listener(
                                                        move |view, _, _, _cx| {
                                                            view.select_profile(profile_id);
                                                            view.open_selected_profile();
                                                        },
                                                    )),
                                                ),
                                        ),
                                ),
                        )
                })),
        )
}

fn main_workspace(
    selected_session: Option<&SessionSnapshot>,
    terminal_frame: Option<&crate::terminal::TerminalFrame>,
    selected_entry: Option<&RemoteEntry>,
    editable_local_path: Option<&str>,
    terminal_focused: bool,
    focus_handle: FocusHandle,
    runtime: Arc<RemoteRuntime>,
    terminal_layout: Arc<Mutex<Option<TerminalLayoutSnapshot>>>,
    window: &Window,
    cx: &mut Context<FtpSftpSshView>,
    dark: bool,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .gap_1()
        .child(file_pane(
            selected_session,
            selected_entry,
            editable_local_path,
            cx,
            dark,
        ))
        .child(terminal_pane(
            selected_session,
            terminal_frame,
            terminal_focused,
            focus_handle,
            runtime,
            terminal_layout,
            window,
            cx,
            dark,
        ))
}

fn file_pane(
    selected_session: Option<&SessionSnapshot>,
    selected_entry: Option<&RemoteEntry>,
    editable_local_path: Option<&str>,
    cx: &mut Context<FtpSftpSshView>,
    dark: bool,
) -> impl IntoElement {
    let file_header = selected_session.map(|snapshot| {
        format!(
            "{} 项 · {}",
            snapshot.remote_entries.len(),
            snapshot.summary.endpoint
        )
    });

    glass_panel(px(392.0), dark)
        .child(panel_header(IconName::FolderOpen, "文件", file_header))
        .child(
            div()
                .h(px(34.0))
                .rounded(px(7.0))
                .bg(glass::inset(dark))
                .border_1()
                .border_color(glass::divider(dark))
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .min_w(px(0.0))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(IconName::FolderOpen)
                                .with_size(ComponentSize::Small)
                                .text_color(theme::semantic().text_secondary),
                        )
                        .child(
                            div()
                                .min_w(px(0.0))
                                .text_size(px(12.0))
                                .line_clamp(1)
                                .font_family(ui::font_mono())
                                .text_color(if selected_session.is_some() {
                                    theme::semantic().text_primary
                                } else {
                                    theme::semantic().text_secondary
                                })
                                .child(
                                    selected_session
                                        .map(|snapshot| snapshot.remote_root.clone())
                                        .unwrap_or_else(|| String::from("未连接")),
                                ),
                        ),
                )
                .child(file_selection_pill(
                    selected_entry,
                    editable_local_path.is_some(),
                    dark,
                )),
        )
        .when(selected_session.is_some(), |col| {
            let snapshot = selected_session.expect("selected session");
            let entries = Arc::new(snapshot.remote_entries.clone());
            let selected_path = selected_entry.map(|entry| entry.path.clone());
            col.child(
                div()
                    .min_h(px(0.0))
                    .flex_1()
                    .rounded(px(8.0))
                    .bg(if dark {
                        hsla(220.0 / 360.0, 0.12, 0.10, 0.18)
                    } else {
                        theme::rgba_with_alpha(theme::white(), 0.42)
                    })
                    .border_1()
                    .border_color(glass::divider(dark))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .drag_over::<ExternalPaths>(move |style, _, _, _| {
                        style
                            .bg(theme::rgba_with_alpha(
                                ui::accent_color(PluginAccent::Cyan),
                                0.10,
                            ))
                            .border_color(theme::rgba_with_alpha(
                                ui::accent_color(PluginAccent::Cyan),
                                0.52,
                            ))
                    })
                    .on_drop(cx.listener(|view, paths: &ExternalPaths, _, cx| {
                        view.upload_paths_into_current_directory(paths.paths().to_vec());
                        cx.stop_propagation();
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|view, event: &MouseDownEvent, _, cx| {
                            view.open_remote_menu(
                                event.position.x.into(),
                                event.position.y.into(),
                                None,
                            );
                            cx.stop_propagation();
                        }),
                    )
                    .child(file_list_header(dark))
                    .child(div().min_h(px(0.0)).flex_1().child(remote_entry_list(
                        entries,
                        selected_path,
                        if snapshot.remote_root == "/" || snapshot.remote_root == "~" {
                            None
                        } else {
                            Some(snapshot.remote_root.clone())
                        },
                        cx,
                        dark,
                    ))),
            )
        })
        .when(selected_session.is_none(), |col| {
            col.child(empty_state(
                IconName::SquareTerminal,
                "打开 session 后显示远端文件列表",
            ))
        })
}

fn remote_entry_list(
    entries: Arc<Vec<RemoteEntry>>,
    selected_path: Option<String>,
    parent_path: Option<String>,
    cx: &mut Context<FtpSftpSshView>,
    dark: bool,
) -> impl IntoElement {
    let has_parent = parent_path.is_some();
    let item_count = entries.len() + if has_parent { 1 } else { 0 };
    div()
        .min_h(px(0.0))
        .size_full()
        .p(px(4.0))
        .when(item_count == 0, |list| {
            list.child(empty_state(IconName::FolderOpen, "当前目录为空"))
        })
        .when(item_count > 0, |list| {
            let entries = entries.clone();
            let selected_path = selected_path.clone();
            let _parent_path = parent_path.clone();
            list.child(
                uniform_list(
                    "remote-entry-list",
                    item_count,
                    cx.processor(move |_view, range: Range<usize>, _window, cx| {
                        range
                            .map(|index| {
                                if has_parent && index == 0 {
                                    remote_parent_row(selected_path.as_deref(), cx, dark)
                                } else {
                                    let entry_index = if has_parent { index - 1 } else { index };
                                    let entry = entries[entry_index].clone();
                                    remote_entry_row(
                                        entry,
                                        index,
                                        selected_path.as_deref(),
                                        cx,
                                        dark,
                                    )
                                }
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .size_full(),
            )
        })
}

fn remote_parent_row(
    selected_path: Option<&str>,
    cx: &mut Context<FtpSftpSshView>,
    dark: bool,
) -> gpui::Stateful<gpui::Div> {
    let selected = selected_path == Some("..");
    div()
        .id(("remote-parent-row", 0usize))
        .w_full()
        .h(px(36.0))
        .rounded(px(6.0))
        .bg(if selected {
            theme::rgba_with_alpha(ui::accent_color(PluginAccent::Cyan), 0.14)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .border_1()
        .border_color(if selected {
            theme::rgba_with_alpha(ui::accent_color(PluginAccent::Cyan), 0.34)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .px_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .cursor_pointer()
        .hover(move |style| style.bg(glass::hover_bg(dark)))
        .on_click(cx.listener(move |view, _, _, _cx| {
            view.go_to_parent_directory();
        }))
        .child(
            div()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap_2()
                .child(file_icon_tile(IconName::ChevronUp, selected, dark))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::NORMAL)
                        .line_clamp(1)
                        .child(".."),
                ),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(12.0))
                .text_color(theme::semantic().text_secondary)
                .child("上级目录"),
        )
}

fn remote_entry_row(
    entry: RemoteEntry,
    index: usize,
    selected_path: Option<&str>,
    cx: &mut Context<FtpSftpSshView>,
    dark: bool,
) -> gpui::Stateful<gpui::Div> {
    let selected = selected_path == Some(entry.path.as_str());
    let remote_path = entry.path.clone();
    let row_path = entry.path.clone();
    let is_dir = entry.is_dir;
    div()
        .id(("remote-entry-row", index))
        .w_full()
        .h(px(36.0))
        .rounded(px(6.0))
        .bg(if selected {
            theme::rgba_with_alpha(ui::accent_color(PluginAccent::Cyan), 0.14)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .border_1()
        .border_color(if selected {
            theme::rgba_with_alpha(ui::accent_color(PluginAccent::Cyan), 0.34)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .px_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .cursor_pointer()
        .hover(move |style| style.bg(glass::hover_bg(dark)))
        .on_click(cx.listener(move |view, event: &gpui::ClickEvent, _, _cx| {
            view.select_remote_entry(remote_path.clone());
            if event.click_count() >= 2 {
                view.open_remote_entry_by_path(&remote_path);
            }
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |view, event: &MouseDownEvent, _, cx| {
                view.select_remote_entry(row_path.clone());
                view.open_remote_menu(
                    event.position.x.into(),
                    event.position.y.into(),
                    Some(row_path.clone()),
                );
                cx.stop_propagation();
            }),
        )
        .child(
            div()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap_2()
                .child(file_icon_tile(
                    if is_dir {
                        IconName::Folder
                    } else {
                        IconName::File
                    },
                    selected,
                    dark,
                ))
                .child(
                    div()
                        .min_w(px(0.0))
                        .text_size(px(13.0))
                        .font_weight(if selected {
                            gpui::FontWeight::MEDIUM
                        } else {
                            gpui::FontWeight::NORMAL
                        })
                        .line_clamp(1)
                        .child(if entry.is_dir {
                            format!("{}/", entry.name)
                        } else {
                            entry.name.clone()
                        }),
                ),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(12.0))
                .text_color(theme::semantic().text_secondary)
                .child(if entry.is_dir {
                    String::from("目录")
                } else {
                    format_bytes(entry.size)
                }),
        )
}

fn terminal_pane(
    selected_session: Option<&SessionSnapshot>,
    terminal_frame: Option<&crate::terminal::TerminalFrame>,
    terminal_focused: bool,
    focus_handle: FocusHandle,
    runtime: Arc<RemoteRuntime>,
    terminal_layout: Arc<Mutex<Option<TerminalLayoutSnapshot>>>,
    _window: &Window,
    cx: &mut Context<FtpSftpSshView>,
    dark: bool,
) -> impl IntoElement {
    let terminal_snapshot = terminal_frame;
    let session_id = selected_session.map(|snapshot| snapshot.summary.session_id.clone());
    let border_color = if terminal_focused {
        theme::rgba_with_alpha(ui::accent_color(PluginAccent::Cyan), 0.34)
    } else {
        theme::rgba_with_alpha(theme::semantic().border_strong, 0.18)
    };
    let terminal_status = selected_session
        .map(|snapshot| {
            if snapshot.summary.has_terminal {
                if terminal_frame.is_some() {
                    "已连接"
                } else {
                    "等待终端"
                }
            } else {
                "无终端"
            }
        })
        .unwrap_or("未连接");

    glass_panel_flex(dark)
        .child(panel_header(
            IconName::SquareTerminal,
            "终端",
            selected_session.map(|snapshot| {
                format!(
                    "{} · {}",
                    snapshot.summary.endpoint, snapshot.connection.message
                )
            }),
        ))
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .rounded(px(8.0))
                .bg(rgb(0x0f1117))
                .border_1()
                .border_color(border_color)
                .flex()
                .flex_col()
                .overflow_hidden()
                .shadow(glass::panel_shadow(dark))
                .child(
                    div()
                        .h(px(28.0))
                        .flex_none()
                        .bg(rgb(0x171a21))
                        .border_b_1()
                        .border_color(theme::rgba_with_alpha(rgb(0x2b313d), 0.72))
                        .px_3()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .min_w(px(0.0))
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(mac_window_dots())
                                .child(
                                    Icon::new(IconName::SquareTerminal)
                                        .with_size(ComponentSize::Small)
                                        .text_color(rgb(0x8aa0b6)),
                                )
                                .child(
                                    div()
                                        .min_w(px(0.0))
                                        .text_size(px(12.0))
                                        .font_family(ui::font_mono())
                                        .line_clamp(1)
                                        .text_color(rgb(0xd4dae4))
                                        .child(
                                            selected_session
                                                .map(|snapshot| {
                                                    format!("{} shell", snapshot.summary.endpoint)
                                                })
                                                .unwrap_or_else(|| String::from("ssh shell")),
                                        ),
                                ),
                        )
                        .child(terminal_status_pill(terminal_status)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h(px(0.0))
                        .p_3()
                        .font_family(ui::font_mono())
                        .text_size(px(14.0))
                        .text_color(rgb(0xe6edf3))
                        .track_focus(&focus_handle)
                        .on_key_down(cx.listener(FtpSftpSshView::handle_terminal_key))
                        .on_children_prepainted({
                            let runtime = runtime.clone();
                            let terminal_layout = terminal_layout.clone();
                            let session_id = session_id.clone();
                            let terminal_snapshot = terminal_snapshot.cloned();
                            move |child_bounds, _, _cx| {
                                let layout =
                                    child_bounds.first().map(|bounds| TerminalLayoutSnapshot {
                                        origin_x: bounds.origin.x.into(),
                                        origin_y: bounds.origin.y.into(),
                                        width: bounds.size.width.into(),
                                        height: bounds.size.height.into(),
                                    });

                                if let Ok(mut stored_layout) = terminal_layout.lock() {
                                    *stored_layout = layout;
                                }

                                if let (Some(session_id), Some(terminal), Some(layout)) =
                                    (session_id.as_ref(), terminal_snapshot.as_ref(), layout)
                                {
                                    sync_terminal_size(
                                        runtime.as_ref(),
                                        session_id,
                                        terminal.columns,
                                        terminal.screen_lines,
                                        layout,
                                    );
                                }
                            }
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(FtpSftpSshView::handle_terminal_mouse_down),
                        )
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(FtpSftpSshView::handle_terminal_mouse_down),
                        )
                        .on_mouse_down(
                            MouseButton::Middle,
                            cx.listener(FtpSftpSshView::handle_terminal_mouse_down),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(FtpSftpSshView::handle_terminal_mouse_up),
                        )
                        .on_mouse_up(
                            MouseButton::Right,
                            cx.listener(FtpSftpSshView::handle_terminal_mouse_up),
                        )
                        .on_mouse_up(
                            MouseButton::Middle,
                            cx.listener(FtpSftpSshView::handle_terminal_mouse_up),
                        )
                        .on_mouse_move(cx.listener(FtpSftpSshView::handle_terminal_mouse_move))
                        .on_scroll_wheel(cx.listener(FtpSftpSshView::handle_terminal_scroll))
                        .overflow_y_scrollbar()
                        .flex()
                        .flex_col()
                        .child(
                            div().size_full().flex().flex_col().gap_0().children(
                                terminal_snapshot
                                    .map(|t| render_terminal_rows(t, true))
                                    .unwrap_or_else(|| {
                                        vec![terminal_empty_state(
                                            if selected_session.is_some_and(|snapshot| {
                                                !snapshot.summary.has_terminal
                                            }) {
                                                "当前协议没有终端"
                                            } else {
                                                "终端未激活"
                                            },
                                        )]
                                    }),
                            ),
                        ),
                ),
        )
}

fn transfer_strip(
    transfers: &[TransferSnapshot],
    notice: String,
    expanded: bool,
    cx: &mut Context<FtpSftpSshView>,
    dark: bool,
) -> impl IntoElement {
    let active = transfers
        .iter()
        .filter(|t| t.status == TransferStatus::Queued || t.status == TransferStatus::Running)
        .count();
    let collapsed_height = px(38.0);
    let expanded_height = if transfers.is_empty() {
        px(72.0)
    } else {
        px(126.0)
    };
    let summary = if active > 0 {
        format!("{active} 个进行中")
    } else if !transfers.is_empty() {
        format!("{} 个完成", transfers.len())
    } else {
        String::from("空闲")
    };
    div()
        .h(if expanded {
            expanded_height
        } else {
            collapsed_height
        })
        .rounded(px(8.0))
        .bg(glass::bar(dark))
        .border_1()
        .border_color(glass::border(dark))
        .shadow(glass::panel_shadow(dark))
        .px_3()
        .py(px(6.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("transfer-toggle")
                                .icon(if expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .ghost()
                                .xsmall()
                                .tooltip(if expanded {
                                    "收起传输队列"
                                } else {
                                    "展开传输队列"
                                })
                                .on_click(cx.listener(|view, _, _, _cx| {
                                    view.transfer_panel_expanded = !view.transfer_panel_expanded;
                                })),
                        )
                        .child(
                            Icon::new(IconName::PanelBottom)
                                .with_size(ComponentSize::Small)
                                .text_color(theme::semantic().text_secondary),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_size(px(14.0))
                                .font_weight(gpui::FontWeight::NORMAL)
                                .child("传输队列")
                                .child(count_pill(transfers.len(), dark)),
                        ),
                )
                .child(
                    div()
                        .min_w(px(0.0))
                        .max_w(px(420.0))
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_2()
                        .child(status_text_pill(&summary, active > 0, dark))
                        .when(expanded && !notice.is_empty(), |row| {
                            row.child(notice_pill(notice.clone(), dark))
                        }),
                ),
        )
        .when(expanded, |root| {
            root.child(
                div()
                    .min_h(px(0.0))
                    .flex_1()
                    .overflow_y_scrollbar()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when(transfers.is_empty(), |col| {
                        col.child(empty_state(IconName::Inbox, "当前没有传输任务"))
                    })
                    .children(transfers.iter().map(|item| {
                        let progress = item.progress_percent().unwrap_or(0);
                        let transfer_id = item.id.clone();
                        div()
                            .rounded(px(7.0))
                            .bg(glass::panel(dark))
                            .border_1()
                            .border_color(glass::divider(dark))
                            .px_3()
                            .py(px(8.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .child(
                                div()
                                    .min_w(px(0.0))
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(transfer_direction_icon(item.direction, dark))
                                            .child(
                                                div()
                                                    .min_w(px(0.0))
                                                    .text_size(px(12.0))
                                                    .font_weight(gpui::FontWeight::NORMAL)
                                                    .line_clamp(1)
                                                    .child(item.remote_path.clone()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(theme::semantic().text_secondary)
                                            .line_clamp(1)
                                            .child(item.local_path.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(progress_bar(progress, dark))
                                    .child(
                                        div()
                                            .w(px(38.0))
                                            .text_size(px(12.0))
                                            .text_color(theme::semantic().text_secondary)
                                            .child(format!("{progress}%")),
                                    )
                                    .child(transfer_status_badge(item.status, dark))
                                    .when(
                                        matches!(
                                            item.status,
                                            TransferStatus::Queued | TransferStatus::Running
                                        ),
                                        |row| {
                                            row.child(
                                                toolbar_button("transfer-cancel", "取消").on_click(
                                                    cx.listener(move |view, _, _, _cx| {
                                                        view.cancel_transfer(transfer_id.clone());
                                                    }),
                                                ),
                                            )
                                        },
                                    ),
                            )
                    })),
            )
        })
}

fn remote_menu_overlay(
    menu: &RemoteMenuState,
    selected_session: Option<&SessionSnapshot>,
    _selected_profile: Option<&Profile>,
    selected_entry: Option<&RemoteEntry>,
    editable_local_path: Option<&str>,
    cx: &mut Context<FtpSftpSshView>,
    dark: bool,
) -> impl IntoElement {
    let target_entry = selected_session.and_then(|snapshot| {
        menu.target_path.as_ref().and_then(|path| {
            snapshot
                .remote_entries
                .iter()
                .find(|entry| &entry.path == path)
        })
    });
    let target_path = target_entry
        .map(|entry| entry.path.clone())
        .or_else(|| menu.target_path.clone());
    let target_is_dir = target_entry.is_some_and(|entry| entry.is_dir);
    let target_editable =
        if let (Some(snapshot), Some(path)) = (selected_session, target_path.as_ref()) {
            snapshot
                .editable_files
                .iter()
                .any(|item| &item.remote_path == path)
        } else {
            editable_local_path.is_some()
        };

    div()
        .size_full()
        .absolute()
        .top_0()
        .left_0()
        .child(
            div()
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .bg(hsla(0.0, 0.0, 0.0, 0.001))
                .id("remote-menu-backdrop")
                .on_click(cx.listener(|view, _, _, _cx| {
                    view.close_remote_menu();
                })),
        )
        .child(
            div()
                .absolute()
                .top(px(menu.y))
                .left(px(menu.x))
                .w(px(180.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(glass::border(dark))
                .bg(glass::panel(dark))
                .shadow(glass::shadow())
                .p_1()
                .flex()
                .flex_col()
                .gap_1()
                .children({
                    let mut items: Vec<AnyElement> = Vec::new();

                    if let Some(path) = target_path.clone() {
                        if target_is_dir {
                            let open_path = path.clone();
                            items.push(
                                menu_item("打开")
                                    .on_click(cx.listener(move |view, _, _, _cx| {
                                        view.close_remote_menu();
                                        view.open_remote_entry_by_path(&open_path);
                                    }))
                                    .into_any_element(),
                            );
                            items.push(
                                menu_item("新建文件夹")
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.close_remote_menu();
                                        view.open_create_directory_prompt(window, cx);
                                    }))
                                    .into_any_element(),
                            );
                        } else {
                            let edit_path = path.clone();
                            items.push(
                                menu_item("编辑")
                                    .on_click(cx.listener(move |view, _, _, _cx| {
                                        view.close_remote_menu();
                                        view.open_remote_entry_by_path(&edit_path);
                                    }))
                                    .into_any_element(),
                            );
                            let download_path = path.clone();
                            items.push(
                                menu_item("下载")
                                    .on_click(cx.listener(move |view, _, _, _cx| {
                                        view.close_remote_menu();
                                        view.select_remote_entry(download_path.clone());
                                        view.download_selected_entry(_cx);
                                    }))
                                    .into_any_element(),
                            );
                            if target_editable {
                                let upload_back_path = path.clone();
                                items.push(
                                    menu_item("回传文件")
                                        .on_click(cx.listener(move |view, _, _, _cx| {
                                            view.close_remote_menu();
                                            view.upload_back_for_path(&upload_back_path);
                                        }))
                                        .into_any_element(),
                                );
                            }
                        }

                        let rename_path = path.clone();
                        items.push(
                            menu_item("重命名")
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.close_remote_menu();
                                    view.open_rename_prompt_for_path(&rename_path, window, cx);
                                }))
                                .into_any_element(),
                        );

                        let delete_path = path.clone();
                        items.push(
                            danger_menu_item("删除")
                                .on_click(cx.listener(move |view, _, _, _cx| {
                                    view.close_remote_menu();
                                    view.open_delete_prompt_for_path(&delete_path);
                                }))
                                .into_any_element(),
                        );
                    } else {
                        items.push(
                            menu_item("新建文件夹")
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.close_remote_menu();
                                    view.open_create_directory_prompt(window, cx);
                                }))
                                .into_any_element(),
                        );
                        items.push(
                            menu_item("上传")
                                .on_click(cx.listener(|view, _, _, _cx| {
                                    view.close_remote_menu();
                                    view.upload_into_current_directory(_cx);
                                }))
                                .into_any_element(),
                        );
                        items.push(
                            menu_item("刷新")
                                .on_click(cx.listener(|view, _, _, _cx| {
                                    view.close_remote_menu();
                                    view.refresh_remote_entries();
                                }))
                                .into_any_element(),
                        );
                        items.push(
                            menu_item("上级目录")
                                .on_click(cx.listener(|view, _, _, _cx| {
                                    view.close_remote_menu();
                                    view.go_to_parent_directory();
                                }))
                                .into_any_element(),
                        );
                    }

                    if selected_entry.is_some() && items.is_empty() {
                        items.push(menu_item("无可用操作").into_any_element());
                    }

                    items
                }),
        )
}

fn profile_menu_overlay(
    menu: &ProfileMenuState,
    cx: &mut Context<FtpSftpSshView>,
    dark: bool,
) -> impl IntoElement {
    let _profile_id = menu.profile_id;
    div()
        .size_full()
        .absolute()
        .top_0()
        .left_0()
        .child(
            div()
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .bg(hsla(0.0, 0.0, 0.0, 0.001))
                .id("profile-menu-backdrop")
                .on_click(cx.listener(|view, _, _, _cx| {
                    view.close_profile_menu();
                })),
        )
        .child(
            div()
                .absolute()
                .top(px(menu.y))
                .left(px(menu.x))
                .w(px(160.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(glass::border(dark))
                .bg(glass::panel(dark))
                .shadow(glass::shadow())
                .p_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    menu_item("连接").on_click(cx.listener(move |view, _, _, _| {
                        view.close_profile_menu();
                        view.open_selected_profile();
                    })),
                )
                .child(
                    menu_item("编辑").on_click(cx.listener(move |view, _, window, cx| {
                        view.close_profile_menu();
                        view.open_profile_editor(window, cx);
                    })),
                )
                .child(
                    danger_menu_item("删除").on_click(cx.listener(move |view, _, _, _cx| {
                        view.close_profile_menu();
                        view.delete_selected_profile();
                    })),
                ),
        )
}

fn remote_action_overlay(
    handle: Entity<FtpSftpSshView>,
    action: &RemoteActionState,
    cx: &mut Context<FtpSftpSshView>,
    dark: bool,
) -> impl IntoElement {
    ui::components::overlay_host(
        dark,
        "remote-action-backdrop",
        move |_, _, app| {
            let _ = app.update_entity(&handle, |view, _cx| {
                view.close_remote_action();
            });
        },
        div()
            .w(px(420.0))
            .rounded(px(10.0))
            .bg(glass::panel(dark))
            .border_1()
            .border_color(glass::border(dark))
            .shadow(glass::shadow())
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_size(px(16.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(match action.kind {
                        RemoteActionKind::CreateDirectory => "新建文件夹",
                        RemoteActionKind::Rename => "重命名",
                        RemoteActionKind::Delete => "删除",
                    }),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .line_height(px(20.0))
                    .text_color(theme::semantic().text_secondary)
                    .child(match action.kind {
                        RemoteActionKind::CreateDirectory => "在当前目录创建新文件夹",
                        RemoteActionKind::Rename => "输入新的名称，目录和文件都支持重命名",
                        RemoteActionKind::Delete => "删除后不可恢复，请确认目标路径",
                    }),
            )
            .when(action.input.is_some(), |col| {
                col.child(labeled_input(
                    "名称",
                    action.input.clone().expect("remote action input"),
                    dark,
                ))
            })
            .when(action.kind == RemoteActionKind::Delete, |col| {
                col.child(
                    div()
                        .rounded(px(8.0))
                        .bg(theme::rgba_with_alpha(theme::semantic().warning, 0.10))
                        .border_1()
                        .border_color(theme::rgba_with_alpha(theme::semantic().warning, 0.26))
                        .p_3()
                        .text_size(px(13.0))
                        .child(action.target_name.clone()),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .child(
                        toolbar_button("remote-action-cancel", "取消").on_click(cx.listener(
                            |view, _, _, _cx| {
                                view.close_remote_action();
                            },
                        )),
                    )
                    .child(
                        toolbar_button("remote-action-confirm", "确认").on_click(cx.listener(
                            |view, _, _, cx| {
                                view.confirm_remote_action(cx);
                            },
                        )),
                    ),
            ),
    )
}

fn profile_editor_overlay(
    handle: Entity<FtpSftpSshView>,
    editor: &ProfileEditorState,
    cx: &mut Context<FtpSftpSshView>,
    dark: bool,
) -> impl IntoElement {
    let protocol = editor.draft.protocol;
    let auth_method = editor.draft.auth.method;
    let is_ssh_family = matches!(protocol, RemoteProtocol::Ssh | RemoteProtocol::Sftp);
    let is_tls_family = matches!(
        protocol,
        RemoteProtocol::FtpsExplicit | RemoteProtocol::FtpsImplicit
    );

    let surface_bg = if dark {
        hsla(220.0 / 360.0, 0.12, 0.14, 0.98)
    } else {
        hsla(220.0 / 360.0, 0.16, 0.985, 0.98)
    };
    let border_color = theme::rgba_with_alpha(theme::semantic().border_default, 0.18);
    let section_accent = theme::rgba_with_alpha(theme::semantic().text_secondary, 1.0);

    let handle_close = handle.clone();
    let handle_backdrop = handle.clone();
    let handle_cancel = handle.clone();

    ui::components::overlay_host(
        dark,
        "profile-editor-backdrop",
        move |_, _, app| {
            let _ = app.update_entity(&handle_backdrop, |view, _cx| {
                view.close_profile_editor();
            });
        },
        div()
            .w(px(600.0))
            .h(px(584.0))
            .rounded(px(14.0))
            .bg(surface_bg)
            .border_1()
            .border_color(border_color)
            .shadow(glass::shadow())
            .flex()
            .flex_col()
            .overflow_hidden()
            // === 标题栏 ===
            .child(
                div()
                    .h(px(52.0))
                    .border_b_1()
                    .border_color(border_color)
                    .px(px(24.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(15.0))
                            .text_color(theme::semantic().text_primary)
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(match editor.mode {
                                ProfileEditorMode::Create => "新建连接",
                                ProfileEditorMode::Edit(_) => "编辑连接",
                            }),
                    )
                    .child(
                        Button::new("editor-close")
                            .icon(IconName::Close)
                            .ghost()
                            .small()
                            .on_click(move |_, _, cx| {
                                let _ = cx.update_entity(&handle_close, |view, _cx| {
                                    view.close_profile_editor();
                                });
                            }),
                    ),
            )
            // === 表单内容 ===
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scrollbar()
                    .px(px(24.0))
                    .py(px(18.0))
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    // == 基本信息 ==
                    .child(editor_section("基本信息", section_accent))
                    .child(labeled_input("名称", editor.name_input.clone(), dark))
                    .child(protocol_selector(protocol, cx))
                    .child(two_col_row(
                        labeled_input("主机", editor.host_input.clone(), dark),
                        labeled_input("端口", editor.port_input.clone(), dark),
                    ))
                    // == 认证 ==
                    .child(editor_section("认证", section_accent))
                    .child(two_col_row(
                        labeled_input("用户名", editor.username_input.clone(), dark),
                        auth_method_selector(auth_method, cx),
                    ))
                    .when(auth_method == AuthMethod::Password, |col| {
                        col.child(labeled_input("密码", editor.password_input.clone(), dark))
                    })
                    .when(auth_method == AuthMethod::PrivateKey, |col| {
                        col.child(labeled_input(
                            "私钥路径",
                            editor.private_key_path_input.clone(),
                            dark,
                        ))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(toolbar_button("profile-key", "选择密钥").on_click(
                                    cx.listener(|view, _, _, cx| {
                                        view.choose_private_key_path(cx);
                                    }),
                                ))
                                .child(div().flex_1().child(labeled_input(
                                    "密钥密码",
                                    editor.private_key_passphrase_input.clone(),
                                    dark,
                                ))),
                        )
                    })
                    // == 路径 ==
                    .child(editor_section("路径", section_accent))
                    .child(two_col_row(
                        labeled_input("远端根目录", editor.remote_root_input.clone(), dark),
                        labeled_input("本地根目录", editor.local_root_input.clone(), dark),
                    ))
                    .child(div().flex().justify_end().child(
                        toolbar_button("profile-local-root", "选择文件夹").on_click(cx.listener(
                            |view, _, _, cx| {
                                view.choose_local_root(cx);
                            },
                        )),
                    ))
                    // == 高级 ==
                    .child(editor_section("高级", section_accent))
                    .child(two_col_row(
                        labeled_input("连接超时（秒）", editor.connect_timeout_input.clone(), dark),
                        labeled_input(
                            "传输并发数",
                            editor.transfer_concurrency_input.clone(),
                            dark,
                        ),
                    ))
                    .child(toggle_row(
                        "FTP 被动模式",
                        editor.draft.limits.passive_mode,
                        "对 FTP / FTPS 生效",
                        "启用",
                        cx,
                    ))
                    .when(is_ssh_family, |col| {
                        col.child(ssh_policy_selector(editor.draft.security.ssh_host_key, cx))
                            .when(
                                editor.draft.security.ssh_host_key
                                    == SshHostKeyPolicy::StrictPinned,
                                |sub| {
                                    sub.child(labeled_input(
                                        "固定主机密钥",
                                        editor.pinned_host_key_input.clone(),
                                        dark,
                                    ))
                                },
                            )
                            .when(
                                editor.draft.security.ssh_host_key
                                    == SshHostKeyPolicy::InsecureAcceptAny,
                                |sub| sub.child(risk_notice("当前 SSH 配置会接受任意主机密钥")),
                            )
                    })
                    .when(is_tls_family, |col| {
                        col.child(tls_policy_selector(editor.draft.security.tls_verify, cx))
                            .when(
                                editor.draft.security.tls_verify == TlsVerifyPolicy::PinnedSha256,
                                |sub| {
                                    sub.child(labeled_input(
                                        "TLS 证书指纹",
                                        editor.pinned_tls_sha256_input.clone(),
                                        dark,
                                    ))
                                },
                            )
                            .when(
                                editor.draft.security.tls_verify
                                    == TlsVerifyPolicy::InsecureAcceptAny,
                                |sub| sub.child(risk_notice("当前 FTPS 配置会跳过证书校验")),
                            )
                    })
                    .child(labeled_multiline("备注", editor.notes_input.clone(), dark))
                    .when(matches!(editor.mode, ProfileEditorMode::Edit(_)), |col| {
                        col.child(
                            div().flex().justify_end().child(
                                Button::new("delete-profile")
                                    .label("删除连接")
                                    .danger()
                                    .on_click(cx.listener(|view, _, _, _cx| {
                                        view.delete_selected_profile();
                                    })),
                            ),
                        )
                    }),
            )
            // === 底部按钮栏 ===
            .child(
                div()
                    .h(px(56.0))
                    .border_t_1()
                    .border_color(border_color)
                    .px(px(24.0))
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .child(
                        mac_action_button("editor-cancel-lite", "取消", false).on_click(
                            move |_, _, cx| {
                                let _ = cx.update_entity(&handle_cancel, |view, _cx| {
                                    view.close_profile_editor();
                                });
                            },
                        ),
                    )
                    .child(
                        mac_action_button("editor-save-lite", "保存", true).on_click(cx.listener(
                            |view, _, _, cx| {
                                view.save_profile_editor(cx);
                            },
                        )),
                    ),
            ),
    )
}

fn mac_action_button(
    id: &'static str,
    label: &'static str,
    primary: bool,
) -> gpui::Stateful<gpui::Div> {
    let mac_blue = rgb(0x007aff);
    div()
        .id(id)
        .h(px(30.0))
        .min_w(px(64.0))
        .rounded(px(7.0))
        .bg(if primary {
            theme::rgba_with_alpha(mac_blue, 0.92)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .border_1()
        .border_color(if primary {
            theme::rgba_with_alpha(mac_blue, 0.22)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .px_3()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(13.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(if primary {
            theme::white()
        } else {
            theme::semantic().text_primary
        })
        .cursor_pointer()
        .hover(move |style| {
            style.bg(if primary {
                theme::rgba_with_alpha(mac_blue, 1.0)
            } else {
                glass::hover_bg(theme_mode::is_dark())
            })
        })
        .child(label)
}

fn editor_section(title: &'static str, color: gpui::Hsla) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(color)
                .child(title),
        )
        .child(div().flex_1().h(px(1.0)).bg(theme::rgba_with_alpha(
            theme::semantic().border_strong,
            0.08,
        )))
}

fn toolbar_button(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(30.0))
        .rounded(px(6.0))
        .bg(glass::inset(theme_mode::is_dark()))
        .border_1()
        .border_color(glass::divider(theme_mode::is_dark()))
        .px_3()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme::semantic().text_primary)
        .cursor_pointer()
        .hover(|style| style.bg(glass::hover_bg(theme_mode::is_dark())))
        .child(label)
}

fn menu_item(label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h(px(32.0))
        .rounded(px(6.0))
        .px_3()
        .flex()
        .items_center()
        .text_size(px(13.0))
        .cursor_pointer()
        .hover(|style| style.bg(glass::hover_bg(theme_mode::is_dark())))
        .child(label)
}

fn danger_menu_item(label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h(px(32.0))
        .rounded(px(6.0))
        .px_3()
        .flex()
        .items_center()
        .text_size(px(13.0))
        .text_color(theme::semantic().danger)
        .cursor_pointer()
        .hover(|style| style.bg(theme::rgba_with_alpha(theme::semantic().danger, 0.08)))
        .child(label)
}

fn labeled_input(label: &'static str, input: Entity<InputState>, dark: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme::semantic().text_secondary)
                .child(label),
        )
        .child(input_shell(input, px(34.0), dark))
}

fn labeled_multiline(
    label: &'static str,
    input: Entity<InputState>,
    dark: bool,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme::semantic().text_secondary)
                .child(label),
        )
        .child(input_shell(input, px(92.0), dark))
}

fn input_shell(input: Entity<InputState>, height: gpui::Pixels, dark: bool) -> impl IntoElement {
    div()
        .h(height)
        .rounded(px(6.0))
        .border_1()
        .border_color(theme::rgba_with_alpha(
            theme::semantic().border_default,
            0.11,
        ))
        .bg(if dark {
            hsla(220.0 / 360.0, 0.12, 0.11, 0.26)
        } else {
            theme::rgba_with_alpha(theme::white(), 0.52)
        })
        .overflow_hidden()
        .child(Input::new(&input).w_full())
}

fn two_col_row(left: impl IntoElement, right: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .items_start()
        .gap_2()
        .child(div().flex_1().child(left))
        .child(div().w(px(218.0)).child(right))
}

fn selector_group(title: &'static str, items: Vec<AnyElement>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme::semantic().text_secondary)
                .child(title),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .rounded(px(8.0))
                .bg(glass::inset(theme_mode::is_dark()))
                .border_1()
                .border_color(theme::rgba_with_alpha(
                    theme::semantic().border_default,
                    0.10,
                ))
                .p(px(3.0))
                .children(items),
        )
}

fn protocol_selector(
    selected: RemoteProtocol,
    cx: &mut Context<FtpSftpSshView>,
) -> impl IntoElement {
    selector_group(
        "协议",
        [
            RemoteProtocol::Ssh,
            RemoteProtocol::Sftp,
            RemoteProtocol::Ftp,
            RemoteProtocol::FtpsExplicit,
            RemoteProtocol::FtpsImplicit,
        ]
        .into_iter()
        .map(|protocol| {
            profile_toggle(protocol.as_str(), protocol.label(), selected == protocol)
                .on_click(cx.listener(move |view, _, window, cx| {
                    view.set_editor_protocol(protocol, window, cx);
                }))
                .into_any_element()
        })
        .collect(),
    )
}

fn auth_method_selector(
    selected: AuthMethod,
    cx: &mut Context<FtpSftpSshView>,
) -> impl IntoElement {
    selector_group(
        "认证方式",
        [
            AuthMethod::Password,
            AuthMethod::PrivateKey,
            AuthMethod::Agent,
        ]
        .into_iter()
        .map(|method| {
            profile_toggle(method.as_str(), method.label(), selected == method)
                .on_click(cx.listener(move |view, _, _, _cx| {
                    view.set_editor_auth_method(method);
                }))
                .into_any_element()
        })
        .collect(),
    )
}

fn ssh_policy_selector(
    selected: SshHostKeyPolicy,
    cx: &mut Context<FtpSftpSshView>,
) -> impl IntoElement {
    selector_group(
        "SSH 安全策略",
        [
            SshHostKeyPolicy::TrustOnFirstUse,
            SshHostKeyPolicy::StrictPinned,
            SshHostKeyPolicy::InsecureAcceptAny,
        ]
        .into_iter()
        .map(|policy| {
            profile_toggle(policy.as_str(), policy.label(), selected == policy)
                .on_click(cx.listener(move |view, _, _, _cx| {
                    view.set_editor_ssh_policy(policy);
                }))
                .into_any_element()
        })
        .collect(),
    )
}

fn tls_policy_selector(
    selected: TlsVerifyPolicy,
    cx: &mut Context<FtpSftpSshView>,
) -> impl IntoElement {
    selector_group(
        "TLS 验证策略",
        [
            TlsVerifyPolicy::SystemRoots,
            TlsVerifyPolicy::PinnedSha256,
            TlsVerifyPolicy::InsecureAcceptAny,
        ]
        .into_iter()
        .map(|policy| {
            profile_toggle(policy.as_str(), policy.label(), selected == policy)
                .on_click(cx.listener(move |view, _, _, _cx| {
                    view.set_editor_tls_policy(policy);
                }))
                .into_any_element()
        })
        .collect(),
    )
}

fn toggle_row(
    label: &'static str,
    enabled: bool,
    hint: &'static str,
    action_label: &'static str,
    cx: &mut Context<FtpSftpSshView>,
) -> impl IntoElement {
    div()
        .rounded(px(8.0))
        .bg(theme::rgba_with_alpha(theme::semantic().bg_elevated, 0.28))
        .border_1()
        .border_color(theme::rgba_with_alpha(
            theme::semantic().border_default,
            0.10,
        ))
        .px_3()
        .py(px(8.0))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(label),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme::semantic().text_secondary)
                        .child(hint),
                ),
        )
        .child(
            profile_toggle("toggle-passive", action_label, enabled).on_click(cx.listener(
                |view, _, _, _cx| {
                    view.toggle_editor_passive_mode();
                },
            )),
        )
}

fn risk_notice(text: &'static str) -> impl IntoElement {
    div()
        .rounded(px(8.0))
        .bg(theme::rgba_with_alpha(theme::semantic().warning, 0.12))
        .border_1()
        .border_color(theme::rgba_with_alpha(theme::semantic().warning, 0.32))
        .px_3()
        .py(px(8.0))
        .text_size(px(13.0))
        .text_color(theme::semantic().warning)
        .child(text)
}

fn profile_toggle(
    id: &'static str,
    label: &'static str,
    selected: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(28.0))
        .rounded(px(6.0))
        .bg(if selected {
            if theme_mode::is_dark() {
                hsla(0.0, 0.0, 1.0, 0.09)
            } else {
                theme::rgba_with_alpha(theme::white(), 0.88)
            }
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .border_1()
        .border_color(if selected {
            theme::rgba_with_alpha(theme::semantic().border_strong, 0.12)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .px_3()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .font_weight(if selected {
            gpui::FontWeight::MEDIUM
        } else {
            gpui::FontWeight::NORMAL
        })
        .text_color(if selected {
            theme::semantic().text_primary
        } else {
            theme::semantic().text_secondary
        })
        .cursor_pointer()
        .hover(|style| style.bg(glass::hover_bg(theme_mode::is_dark())))
        .child(label)
}

fn render_terminal_rows(terminal: &crate::terminal::TerminalFrame, dark: bool) -> Vec<gpui::Div> {
    terminal
        .styled_rows
        .iter()
        .enumerate()
        .map(|(row_index, row)| render_terminal_row(row, row_index, &terminal.cursor, dark))
        .collect()
}

/// 按「连续同 style 段（run）」合并渲染一行：相邻且样式相同的 cell 合并成一个文本元素，
/// 光标所在 cell 单独成段（反色）并打断 run。div 数从 O(列数) 降到 O(每行 run 数)。
fn render_terminal_row(
    row: &crate::terminal::TerminalStyledRow,
    row_index: usize,
    cursor: &crate::terminal::TerminalCursor,
    dark: bool,
) -> gpui::Div {
    let mut line = div()
        .flex()
        .items_center()
        .gap_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .line_height(px(14.0));
    let cursor_in_row = cursor.visible && cursor.row == row_index;
    let mut index = 0;
    while index < row.cells.len() {
        if cursor_in_row && cursor.column == index {
            let cell = &row.cells[index];
            line = line.child(render_terminal_run(&cell.text, cell.style, true, dark));
            index += 1;
            continue;
        }
        let style = row.cells[index].style;
        let mut text = String::new();
        while index < row.cells.len()
            && !(cursor_in_row && cursor.column == index)
            && row.cells[index].style == style
        {
            text.push_str(&row.cells[index].text);
            index += 1;
        }
        line = line.child(render_terminal_run(&text, style, false, dark));
    }
    line
}

fn render_terminal_run(
    text: &str,
    style: TerminalCellStyle,
    cursor_here: bool,
    dark: bool,
) -> gpui::Div {
    // 浅色模式：将终端默认暗色主题的前景/背景替换为浅色
    // 0x111317 = alacritty 默认 Background，0xeaeaea = 默认 Foreground
    let fg_val = if !dark && style.fg == 0xeaeaea {
        0x1a1a1a
    } else {
        style.fg
    };
    let bg_val = if !dark && style.bg == 0x111317 {
        0xffffff
    } else {
        style.bg
    };
    let fg = rgb(fg_val);
    let bg = rgb(bg_val);
    let cursor_fg = if dark { rgb(0x111317) } else { rgb(0xffffff) };
    let cursor_bg = if dark { theme::white() } else { rgb(0x1a1a1a) };
    let mut run = div()
        .flex_none()
        .flex_shrink_0()
        .whitespace_nowrap()
        .text_color(if cursor_here { cursor_fg } else { fg })
        .text_bg(if cursor_here { cursor_bg } else { bg })
        .child(text.to_string());

    if style.bold {
        run = run.font_weight(gpui::FontWeight::BOLD);
    }
    if style.italic {
        run = run.italic();
    }
    if style.underline {
        run = run.underline();
    }
    if style.strike {
        run = run.line_through();
    }

    run
}

fn sync_terminal_size(
    runtime: &RemoteRuntime,
    session_id: &SessionId,
    current_columns: usize,
    current_rows: usize,
    layout: TerminalLayoutSnapshot,
) {
    const TERMINAL_CELL_WIDTH_PX: f32 = 7.3;
    const TERMINAL_CELL_HEIGHT_PX: f32 = 14.0;

    let estimated_columns = (layout.width / TERMINAL_CELL_WIDTH_PX).floor().max(40.0) as u32;
    let estimated_rows = (layout.height / TERMINAL_CELL_HEIGHT_PX).floor().max(12.0) as u32;
    if estimated_columns as usize == current_columns && estimated_rows as usize == current_rows {
        return;
    }
    let _ = runtime.resize_terminal(session_id, estimated_columns, estimated_rows);
}

fn protocol_badge(label: &'static str, selected: bool, dark: bool) -> impl IntoElement {
    let color = if selected {
        theme::semantic().text_primary
    } else {
        theme::semantic().text_secondary
    };
    div()
        .h(px(17.0))
        .min_w(px(36.0))
        .rounded(px(5.0))
        .bg(if selected {
            theme::rgba_with_alpha(theme::semantic().bg_elevated, 0.46)
        } else {
            glass::inset(dark)
        })
        .border_1()
        .border_color(if selected {
            theme::rgba_with_alpha(theme::semantic().border_strong, 0.12)
        } else {
            glass::divider(dark)
        })
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(color)
        .child(label)
}

fn status_pill(has_session: bool, dark: bool) -> impl IntoElement {
    let color = if has_session {
        theme::semantic().success
    } else if dark {
        rgb(0x667085)
    } else {
        rgb(0x9aa3af)
    };
    div()
        .h(px(22.0))
        .w(px(12.0))
        .flex()
        .items_center()
        .justify_center()
        .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(color))
}

fn file_selection_pill(
    selected_entry: Option<&RemoteEntry>,
    editable: bool,
    dark: bool,
) -> impl IntoElement {
    let (label, color) = selected_entry
        .map(|entry| {
            if entry.is_dir {
                ("目录", ui::accent_color(PluginAccent::Cyan))
            } else if editable {
                ("可编辑", theme::semantic().success)
            } else {
                ("文件", theme::semantic().text_secondary)
            }
        })
        .unwrap_or(("空闲", theme::semantic().text_secondary));

    div()
        .h(px(22.0))
        .min_w(px(44.0))
        .rounded(px(5.0))
        .bg(if selected_entry.is_some() {
            theme::rgba_with_alpha(color, 0.08)
        } else {
            glass::inset(dark)
        })
        .border_1()
        .border_color(if selected_entry.is_some() {
            theme::rgba_with_alpha(color, 0.14)
        } else {
            glass::divider(dark)
        })
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(color)
        .child(label)
}

fn file_list_header(dark: bool) -> impl IntoElement {
    div()
        .h(px(28.0))
        .flex_none()
        .border_b_1()
        .border_color(glass::divider(dark))
        .px_3()
        .flex()
        .items_center()
        .justify_between()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme::semantic().text_secondary)
        .child("名称")
        .child("类型 / 大小")
}

fn file_icon_tile(icon: IconName, selected: bool, _dark: bool) -> impl IntoElement {
    div()
        .w(px(20.0))
        .h(px(20.0))
        .flex_none()
        .rounded(px(6.0))
        .bg(if selected {
            theme::rgba_with_alpha(ui::accent_color(PluginAccent::Cyan), 0.10)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .border_1()
        .border_color(if selected {
            theme::rgba_with_alpha(ui::accent_color(PluginAccent::Cyan), 0.12)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .flex()
        .items_center()
        .justify_center()
        .child(
            Icon::new(icon)
                .with_size(ComponentSize::Small)
                .text_color(if selected {
                    ui::accent_color(PluginAccent::Cyan)
                } else {
                    theme::semantic().text_secondary
                }),
        )
}

fn terminal_status_pill(text: &'static str) -> impl IntoElement {
    let color = match text {
        "已连接" => theme::semantic().success,
        "无终端" => theme::semantic().warning,
        _ => rgb(0x94a3b8),
    };
    div()
        .h(px(20.0))
        .rounded(px(6.0))
        .bg(theme::rgba_with_alpha(color, 0.12))
        .border_1()
        .border_color(theme::rgba_with_alpha(color, 0.24))
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(color)
        .child(text)
}

fn mac_window_dots() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(rgb(0xff5f57)))
        .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(rgb(0xffbd2e)))
        .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(rgb(0x28c840)))
}

fn terminal_empty_state(text: &'static str) -> gpui::Div {
    div()
        .size_full()
        .flex()
        .items_start()
        .justify_start()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(rgb(0x8fd8e8))
                        .child("$"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(0x94a3b8))
                        .child(text),
                ),
        )
}

fn count_pill(count: usize, dark: bool) -> impl IntoElement {
    div()
        .h(px(18.0))
        .min_w(px(26.0))
        .rounded(px(5.0))
        .bg(glass::inset(dark))
        .border_1()
        .border_color(glass::divider(dark))
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme::semantic().text_secondary)
        .child(count.to_string())
}

fn status_text_pill(summary: &str, active: bool, dark: bool) -> impl IntoElement {
    let color = if active {
        theme::semantic().info
    } else {
        theme::semantic().text_secondary
    };
    div()
        .h(px(24.0))
        .rounded(px(6.0))
        .bg(if active {
            theme::rgba_with_alpha(color, 0.10)
        } else {
            glass::inset(dark)
        })
        .border_1()
        .border_color(if active {
            theme::rgba_with_alpha(color, 0.20)
        } else {
            glass::divider(dark)
        })
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(color)
        .child(summary.to_string())
}

fn notice_pill(notice: String, dark: bool) -> impl IntoElement {
    div()
        .h(px(24.0))
        .max_w(px(300.0))
        .rounded(px(6.0))
        .bg(glass::inset(dark))
        .border_1()
        .border_color(glass::divider(dark))
        .px_2()
        .flex()
        .items_center()
        .gap_1()
        .child(
            Icon::new(IconName::Info)
                .with_size(ComponentSize::Small)
                .text_color(theme::semantic().text_secondary),
        )
        .child(
            div()
                .min_w(px(0.0))
                .line_clamp(1)
                .text_size(px(11.0))
                .text_color(theme::semantic().text_secondary)
                .child(notice),
        )
}

fn transfer_direction_icon(direction: TransferDirection, dark: bool) -> impl IntoElement {
    let icon = match direction {
        TransferDirection::Upload => IconName::ArrowUp,
        TransferDirection::Download => IconName::ArrowDown,
    };
    div()
        .w(px(22.0))
        .h(px(22.0))
        .flex_none()
        .rounded(px(6.0))
        .bg(glass::inset(dark))
        .border_1()
        .border_color(glass::divider(dark))
        .flex()
        .items_center()
        .justify_center()
        .child(
            Icon::new(icon)
                .with_size(ComponentSize::Small)
                .text_color(ui::accent_color(PluginAccent::Cyan)),
        )
}

fn progress_bar(progress: u8, dark: bool) -> impl IntoElement {
    div()
        .w(px(84.0))
        .h(px(6.0))
        .rounded_full()
        .bg(glass::inset(dark))
        .overflow_hidden()
        .child(
            div()
                .h_full()
                .w(px((progress as f32 / 100.0) * 84.0))
                .rounded_full()
                .bg(ui::accent_color(PluginAccent::Cyan)),
        )
}

fn transfer_status_badge(status: TransferStatus, dark: bool) -> impl IntoElement {
    let (label, color) = match status {
        TransferStatus::Queued => ("排队中", theme::semantic().text_secondary),
        TransferStatus::Running => ("传输中", theme::semantic().info),
        TransferStatus::Completed => ("完成", theme::semantic().success),
        TransferStatus::Failed => ("失败", theme::semantic().danger),
        TransferStatus::Cancelled => ("已取消", theme::semantic().warning),
    };
    div()
        .h(px(24.0))
        .w(px(54.0))
        .rounded(px(6.0))
        .bg(if status == TransferStatus::Queued {
            glass::inset(dark)
        } else {
            theme::rgba_with_alpha(color, 0.10)
        })
        .border_1()
        .border_color(if status == TransferStatus::Queued {
            glass::divider(dark)
        } else {
            theme::rgba_with_alpha(color, 0.22)
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(color)
        .child(label)
}

fn glass_panel(width: gpui::Pixels, dark: bool) -> gpui::Div {
    div()
        .w(width)
        .min_h(px(0.0))
        .rounded(px(8.0))
        .bg(glass::panel(dark))
        .border_1()
        .border_color(glass::border(dark))
        .shadow(glass::panel_shadow(dark))
        .p(px(8.0))
        .flex()
        .flex_col()
        .gap_1()
}

fn glass_panel_flex(dark: bool) -> gpui::Div {
    div()
        .flex_1()
        .min_h(px(0.0))
        .rounded(px(8.0))
        .bg(glass::panel(dark))
        .border_1()
        .border_color(glass::border(dark))
        .shadow(glass::panel_shadow(dark))
        .p(px(8.0))
        .flex()
        .flex_col()
        .gap_1()
}

fn panel_header(icon: IconName, title: &'static str, subtitle: Option<String>) -> impl IntoElement {
    div()
        .flex_none()
        .h(px(40.0))
        .px_2()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(icon)
                        .with_size(ComponentSize::Small)
                        .text_color(theme::semantic().text_secondary),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(title),
                        )
                        .children(subtitle.map(|subtitle| {
                            div()
                                .text_size(px(11.0))
                                .line_clamp(1)
                                .text_color(theme::semantic().text_secondary)
                                .child(subtitle)
                        })),
                ),
        )
}

fn empty_state(icon: IconName, text: &'static str) -> impl IntoElement {
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .items_center()
        .justify_center()
        .p_3()
        .child(
            div()
                .min_w(px(0.0))
                .max_w(px(230.0))
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .text_color(theme::semantic().text_secondary)
                .child(
                    Icon::new(icon)
                        .with_size(ComponentSize::Medium)
                        .text_color(theme::semantic().text_secondary),
                )
                .child(div().text_size(px(12.0)).line_height(px(16.0)).child(text)),
        )
}

fn dialog_directory_from_text(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let expanded = expand_dialog_path(trimmed);
    if expanded.is_dir() {
        Some(expanded)
    } else {
        expanded
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
    }
}

fn expand_dialog_path(text: &str) -> PathBuf {
    if text == "~" {
        if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
            return PathBuf::from(home);
        }
    }
    if let Some(rest) = text.strip_prefix("~/").or_else(|| text.strip_prefix("~\\"))
        && let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(text)
}

fn build_profile_editor_state(
    mode: ProfileEditorMode,
    draft: ProfileDraft,
    window: &mut Window,
    cx: &mut Context<FtpSftpSshView>,
) -> ProfileEditorState {
    let draft = draft.normalize();
    let remote_root_placeholder = draft.protocol.default_remote_root();
    ProfileEditorState {
        mode,
        name_input: compact_input(window, cx, &draft.name, "连接名称", false),
        host_input: compact_input(window, cx, &draft.host, "example.com", false),
        port_input: compact_input(window, cx, &draft.port.to_string(), "22", false),
        username_input: compact_input(window, cx, &draft.auth.username, "用户名", false),
        password_input: compact_input(window, cx, &draft.auth.password, "密码", false),
        private_key_path_input: compact_input(
            window,
            cx,
            &draft.auth.private_key_path,
            "~/.ssh/id_ed25519",
            false,
        ),
        private_key_passphrase_input: compact_input(
            window,
            cx,
            &draft.auth.private_key_passphrase,
            "私钥口令",
            false,
        ),
        remote_root_input: compact_input(
            window,
            cx,
            &draft.paths.remote_root,
            remote_root_placeholder,
            false,
        ),
        local_root_input: compact_input(window, cx, &draft.paths.local_root, "~/Downloads", false),
        connect_timeout_input: compact_input(
            window,
            cx,
            &draft.limits.connect_timeout_secs.to_string(),
            "15",
            false,
        ),
        transfer_concurrency_input: compact_input(
            window,
            cx,
            &draft.limits.transfer_concurrency.to_string(),
            "3",
            false,
        ),
        pinned_host_key_input: compact_input(
            window,
            cx,
            &draft.security.pinned_host_key,
            "ssh-ed25519 AAAA...",
            false,
        ),
        pinned_tls_sha256_input: compact_input(
            window,
            cx,
            &draft.security.pinned_tls_sha256,
            "sha256:abcd...",
            false,
        ),
        notes_input: compact_input(window, cx, &draft.notes, "备注 / 环境说明", true),
        draft,
    }
}

fn compact_input(
    window: &mut Window,
    cx: &mut Context<FtpSftpSshView>,
    value: &str,
    placeholder: &str,
    _multiline: bool,
) -> Entity<InputState> {
    let value = value.to_string();
    let placeholder = placeholder.to_string();
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .default_value(value)
    })
}

fn join_remote_path(parent: &str, child: &str) -> String {
    let trimmed_child = child.trim_matches('/');
    if parent == "/" {
        format!("/{trimmed_child}")
    } else if parent == "~" {
        format!("~/{trimmed_child}")
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), trimmed_child)
    }
}

fn remote_parent_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "/" || trimmed == "~" {
        return trimmed.to_string().if_empty("/");
    }
    if trimmed.starts_with("~/") {
        let mut parts: Vec<&str> = trimmed.split('/').collect();
        let _ = parts.pop();
        if parts.len() <= 1 {
            String::from("~")
        } else {
            parts.join("/")
        }
    } else {
        let mut parts: Vec<&str> = trimmed.split('/').filter(|part| !part.is_empty()).collect();
        let _ = parts.pop();
        if parts.is_empty() {
            String::from("/")
        } else {
            format!("/{}", parts.join("/"))
        }
    }
}

trait StringEmptyExt {
    fn if_empty(self, fallback: &str) -> String;
}

impl StringEmptyExt for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

fn terminal_input_for_event(
    event: &KeyDownEvent,
    cx: &mut Context<FtpSftpSshView>,
) -> Option<TerminalInput> {
    let key = event.keystroke.key.as_str();
    let modifiers = event.keystroke.modifiers;

    // Platform modifier（macOS 上为 Cmd，Win/Linux 上为 Win/Super）
    // 作为 OS/应用级快捷键，默认不发送到终端。
    // 例外：Cmd+V / Ctrl+V → 粘贴剪贴板。
    if modifiers.platform {
        if key == "v" {
            return cx
                .read_from_clipboard()
                .and_then(|item| item.text())
                .map(TerminalInput::Paste);
        }
        return None;
    }

    // Named 特殊按键映射
    let named = match key {
        "enter" => Some(TerminalInput::Enter),
        "tab" if modifiers.shift => Some(TerminalInput::ShiftTab),
        "tab" => Some(TerminalInput::Tab),
        "backspace" => Some(TerminalInput::Backspace),
        "delete" => Some(TerminalInput::Delete),
        "insert" => Some(TerminalInput::Insert),
        "escape" => Some(TerminalInput::Escape),
        "home" => Some(TerminalInput::Home),
        "end" => Some(TerminalInput::End),
        "pageup" => Some(TerminalInput::PageUp),
        "pagedown" => Some(TerminalInput::PageDown),
        "up" => Some(TerminalInput::ArrowUp),
        "down" => Some(TerminalInput::ArrowDown),
        "left" => Some(TerminalInput::ArrowLeft),
        "right" => Some(TerminalInput::ArrowRight),
        _ => key
            .strip_prefix('f')
            .and_then(|value| value.parse::<u8>().ok())
            .map(TerminalInput::Function),
    };
    if named.is_some() {
        return named;
    }

    // 修饰键组合（Ctrl、Alt、Ctrl+Alt）
    if modifiers.control && modifiers.alt {
        return terminal_modified_char(key).map(TerminalInput::CtrlAlt);
    }
    if modifiers.control && key == "v" {
        return cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .map(TerminalInput::Paste);
    }
    if modifiers.control {
        return terminal_modified_char(key).map(TerminalInput::Ctrl);
    }
    if modifiers.alt {
        return terminal_modified_char(key).map(TerminalInput::Alt);
    }
    if modifiers.function {
        return None;
    }

    // 普通文本输入
    let key_char = event.keystroke.key_char.as_deref();
    key_char
        .or_else(|| terminal_text_for_key(key))
        .filter(|text| !text.is_empty())
        .map(|text| TerminalInput::Text(text.to_string()))
}

fn terminal_text_for_key(key: &str) -> Option<&str> {
    if key == "space" {
        Some(" ")
    } else if key.chars().count() == 1 {
        Some(key)
    } else {
        None
    }
}

fn terminal_modified_char(key: &str) -> Option<char> {
    let mut chars = key.chars();
    let ch = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(ch.to_ascii_lowercase())
}

fn terminal_mouse_press_input(
    event: &MouseDownEvent,
    frame: &crate::terminal::TerminalFrame,
    layout: TerminalLayoutSnapshot,
) -> Option<TerminalInput> {
    let (column, row) = terminal_grid_position(event.position.x, event.position.y, frame, layout)?;
    Some(TerminalInput::MouseButton {
        button: map_mouse_button(event.button)?,
        column,
        row,
        kind: TerminalMouseEventKind::Press,
        modifiers: map_mouse_modifiers(event.modifiers),
    })
}

fn terminal_mouse_release_input(
    event: &MouseUpEvent,
    frame: &crate::terminal::TerminalFrame,
    layout: TerminalLayoutSnapshot,
) -> Option<TerminalInput> {
    let (column, row) = terminal_grid_position(event.position.x, event.position.y, frame, layout)?;
    Some(TerminalInput::MouseButton {
        button: map_mouse_button(event.button)?,
        column,
        row,
        kind: TerminalMouseEventKind::Release,
        modifiers: map_mouse_modifiers(event.modifiers),
    })
}

fn terminal_mouse_move_input(
    event: &MouseMoveEvent,
    frame: &crate::terminal::TerminalFrame,
    layout: TerminalLayoutSnapshot,
) -> Option<TerminalInput> {
    let (column, row) = terminal_grid_position(event.position.x, event.position.y, frame, layout)?;
    Some(TerminalInput::MouseMove {
        button: event.pressed_button.and_then(map_mouse_button),
        column,
        row,
        modifiers: map_mouse_modifiers(event.modifiers),
    })
}

fn terminal_mouse_scroll_input(
    event: &ScrollWheelEvent,
    frame: &crate::terminal::TerminalFrame,
    layout: TerminalLayoutSnapshot,
) -> Option<TerminalInput> {
    let (column, row) = terminal_grid_position(event.position.x, event.position.y, frame, layout)?;
    let delta = match event.delta {
        ScrollDelta::Pixels(delta) => {
            let value: f32 = delta.y.into();
            value
        }
        ScrollDelta::Lines(delta) => delta.y,
    };
    let direction = if delta < 0.0 {
        TerminalMouseScrollDirection::Down
    } else {
        TerminalMouseScrollDirection::Up
    };
    Some(TerminalInput::MouseScroll {
        direction,
        column,
        row,
        modifiers: map_mouse_modifiers(event.modifiers),
    })
}

fn terminal_grid_position(
    x: gpui::Pixels,
    y: gpui::Pixels,
    frame: &crate::terminal::TerminalFrame,
    layout: TerminalLayoutSnapshot,
) -> Option<(u16, u16)> {
    let x_px: f32 = x.into();
    let y_px: f32 = y.into();
    if x_px < layout.origin_x
        || y_px < layout.origin_y
        || x_px > layout.origin_x + layout.width
        || y_px > layout.origin_y + layout.height
    {
        return None;
    }
    let local_x = (x_px - layout.origin_x).max(0.0);
    let local_y = (y_px - layout.origin_y).max(0.0);
    let cell_width = (layout.width / frame.columns.max(1) as f32).max(1.0);
    let cell_height = (layout.height / frame.screen_lines.max(1) as f32).max(1.0);
    let column = ((local_x / cell_width).floor() as usize).min(frame.columns.saturating_sub(1)) + 1;
    let row =
        ((local_y / cell_height).floor() as usize).min(frame.screen_lines.saturating_sub(1)) + 1;
    Some((column as u16, row as u16))
}

fn map_mouse_button(button: MouseButton) -> Option<TerminalMouseButton> {
    match button {
        MouseButton::Left => Some(TerminalMouseButton::Left),
        MouseButton::Right => Some(TerminalMouseButton::Right),
        MouseButton::Middle => Some(TerminalMouseButton::Middle),
        _ => None,
    }
}

fn map_mouse_modifiers(modifiers: gpui::Modifiers) -> TerminalMouseModifiers {
    TerminalMouseModifiers {
        shift: modifiers.shift,
        alt: modifiers.alt,
        ctrl: modifiers.control,
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[allow(dead_code)]
fn _into_pathbuf(value: &str) -> PathBuf {
    PathBuf::from(value)
}

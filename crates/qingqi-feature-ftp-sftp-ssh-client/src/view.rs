use std::{collections::HashMap, path::Path, sync::Arc};

use crate::{
    model::{
        AuthMethod, ConnectionStatus, ProtocolLogEntry, ProtocolLogKind, RemoteEditDraft,
        RemoteEditState, RemoteFileItem, RemoteProfile, RemoteProfileDraft, RemoteProtocol,
        RightPanelMode, SessionSummary, SessionTransferItem, TerminalSnapshot, TransferItem,
        TransferStatus,
    },
    service::{FtpSftpSshService, looks_like_text_file_name},
    transfer::transfer_counts,
};
use gpui::{
    App, AppContext, Context, Entity, ExternalPaths, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, Point, Render, SharedString, StatefulInteractiveElement,
    Styled, Window, div, hsla, prelude::FluentBuilder, px,
};
use gpui_component::{Icon, IconName, Sizable};
use qingqi_ui::{
    text_input::{TextInput, TextInputStyle},
    theme, ui,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProfileEditorMode {
    Existing(i64),
    New,
}

#[derive(Clone)]
struct ProfileEditorInputs {
    name: Entity<TextInput>,
    host: Entity<TextInput>,
    port: Entity<TextInput>,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    private_key_path: Entity<TextInput>,
    private_key_passphrase: Entity<TextInput>,
    remote_dir: Entity<TextInput>,
    local_dir: Entity<TextInput>,
    encoding: Entity<TextInput>,
    connect_timeout_secs: Entity<TextInput>,
    jump_host: Entity<TextInput>,
    jump_port: Entity<TextInput>,
    jump_username: Entity<TextInput>,
    jump_password: Entity<TextInput>,
    jump_private_key_path: Entity<TextInput>,
    jump_private_key_passphrase: Entity<TextInput>,
    notes: Entity<TextInput>,
}

#[derive(Clone)]
struct ProfileEditorSnapshot {
    inputs: ProfileEditorInputs,
    mode: ProfileEditorMode,
    protocol: RemoteProtocol,
    auth_method: AuthMethod,
    passive_mode: bool,
    jump_enabled: bool,
    pinned: bool,
    notice: String,
    show_advanced: bool,
}

#[derive(Clone)]
struct ProfileMenuState {
    profile: RemoteProfile,
    position: Point<Pixels>,
}

#[derive(Clone)]
struct FileMenuState {
    item: RemoteFileItem,
    position: Point<Pixels>,
}

pub struct FtpSftpSshView {
    service: Arc<FtpSftpSshService>,
    profiles_cache: Vec<RemoteProfile>,
    selected_profile_cache: Option<RemoteProfile>,
    service_revision: u64,
    search_input: Option<Entity<TextInput>>,
    terminal_input: Option<Entity<TextInput>>,
    new_folder_input: Option<Entity<TextInput>>,
    editor_inputs: Option<ProfileEditorInputs>,
    editor_mode: ProfileEditorMode,
    editor_protocol: RemoteProtocol,
    editor_auth_method: AuthMethod,
    editor_passive_mode: bool,
    editor_jump_enabled: bool,
    editor_pinned: bool,
    editor_notice: String,
    editor_open: bool,
    editor_show_advanced: bool,
    folder_sheet_open: bool,
    show_global_transfers: bool,
    transfer_collapsed: bool,
    profile_menu: Option<ProfileMenuState>,
    file_menu: Option<FileMenuState>,
}

impl FtpSftpSshView {
    fn log_ui_action(&self, action: &'static str) {
        tracing::info!(target: "ftp_sftp_ssh.ui", action, "ftp/sftp/ssh ui action");
    }

    fn log_ui_action_with_profile(&self, action: &'static str, profile_id: i64) {
        tracing::info!(
            target: "ftp_sftp_ssh.ui",
            action,
            profile_id,
            "ftp/sftp/ssh ui action"
        );
    }

    pub fn new(service: Arc<FtpSftpSshService>) -> Self {
        let selected = service.selected_profile().ok().flatten();
        let profiles_cache = service.list_profiles().unwrap_or_default();
        let draft = selected
            .as_ref()
            .map(RemoteProfileDraft::from_profile)
            .unwrap_or_else(RemoteProfileDraft::blank);
        Self {
            service,
            profiles_cache,
            selected_profile_cache: selected.clone(),
            service_revision: 0,
            search_input: None,
            terminal_input: None,
            new_folder_input: None,
            editor_inputs: None,
            editor_mode: selected
                .as_ref()
                .map(|profile| ProfileEditorMode::Existing(profile.id))
                .unwrap_or(ProfileEditorMode::New),
            editor_protocol: draft.protocol,
            editor_auth_method: draft.auth_method,
            editor_passive_mode: draft.passive_mode,
            editor_jump_enabled: draft.jump_enabled,
            editor_pinned: draft.pinned,
            editor_notice: String::from("常用配置默认展开，高级配置可折叠"),
            editor_open: false,
            editor_show_advanced: false,
            folder_sheet_open: false,
            show_global_transfers: false,
            transfer_collapsed: false,
            profile_menu: None,
            file_menu: None,
        }
    }

    fn refresh_cached_profiles_if_needed(&mut self) {
        let revision = self.service.revision();
        if revision == self.service_revision {
            return;
        }
        self.profiles_cache = self.service.list_profiles().unwrap_or_default();
        self.selected_profile_cache = self.service.selected_profile().ok().flatten();
        self.service_revision = revision;
    }

    fn ensure_inputs(&mut self, cx: &mut Context<Self>) {
        if self.search_input.is_none() {
            self.search_input = Some(cx.new(|cx| {
                let mut input = TextInput::new(cx, "搜索连接 / 主机 / 协议", "");
                input.set_chrome(false, cx);
                input.set_style(
                    TextInputStyle {
                        height: 30.0,
                        font_size: 11.0,
                        padding: 7.0,
                    },
                    cx,
                );
                input
            }));
        }
        if self.terminal_input.is_none() {
            self.terminal_input = Some(cx.new(|cx| {
                let mut input = TextInput::new(cx, "command", "");
                input.set_chrome(false, cx);
                input.set_monospace(true, cx);
                input.set_style(
                    TextInputStyle {
                        height: 24.0,
                        font_size: 11.0,
                        padding: 6.0,
                    },
                    cx,
                );
                input
            }));
        }
        if self.new_folder_input.is_none() {
            self.new_folder_input = Some(profile_input(cx, "新建文件夹", "", false, 34.0));
        }
    }

    fn ensure_editor_inputs(&mut self, cx: &mut Context<Self>) {
        if self.editor_inputs.is_some() {
            return;
        }
        self.refresh_cached_profiles_if_needed();
        let draft = match self.editor_mode {
            ProfileEditorMode::Existing(id) => self
                .profiles()
                .into_iter()
                .find(|profile| profile.id == id)
                .map(|profile| RemoteProfileDraft::from_profile(&profile))
                .unwrap_or_else(RemoteProfileDraft::blank),
            ProfileEditorMode::New => RemoteProfileDraft::blank(),
        };
        self.editor_inputs = Some(ProfileEditorInputs::from_draft(cx, &draft));
    }

    fn profiles(&self) -> Vec<RemoteProfile> {
        self.profiles_cache.clone()
    }

    fn selected_profile(&self) -> Option<RemoteProfile> {
        self.selected_profile_cache.clone()
    }

    fn search_text(&self, cx: &Context<Self>) -> String {
        self.search_input
            .as_ref()
            .map(|input| input.read(cx).text())
            .unwrap_or_default()
    }

    fn filtered_profiles(&self, cx: &Context<Self>) -> Vec<RemoteProfile> {
        let query = self.search_text(cx).trim().to_ascii_lowercase();
        let mut profiles = self.profiles();
        if query.is_empty() {
            return profiles;
        }
        profiles.retain(|profile| {
            let haystack = format!(
                "{} {} {} {} {}",
                profile.name,
                profile.host,
                profile.username,
                profile.protocol.label(),
                profile.notes
            )
            .to_ascii_lowercase();
            haystack.contains(&query)
        });
        profiles
    }

    fn editor_snapshot(&self) -> Option<ProfileEditorSnapshot> {
        Some(ProfileEditorSnapshot {
            inputs: self.editor_inputs.as_ref()?.clone(),
            mode: self.editor_mode,
            protocol: self.editor_protocol,
            auth_method: self.editor_auth_method,
            passive_mode: self.editor_passive_mode,
            jump_enabled: self.editor_jump_enabled,
            pinned: self.editor_pinned,
            notice: self.editor_notice.clone(),
            show_advanced: self.editor_show_advanced,
        })
    }

    fn begin_new_profile(&mut self, cx: &mut Context<Self>) {
        self.log_ui_action("begin_new_profile");
        self.refresh_cached_profiles_if_needed();
        self.editor_mode = ProfileEditorMode::New;
        self.editor_open = true;
        self.editor_show_advanced = false;
        self.profile_menu = None;
        self.file_menu = None;
        self.load_editor_draft(RemoteProfileDraft::blank(), cx);
        self.editor_notice = String::from("正在新建连接配置");
    }

    fn open_profile_editor(&mut self, id: i64, cx: &mut Context<Self>) {
        self.log_ui_action_with_profile("open_profile_editor", id);
        self.refresh_cached_profiles_if_needed();
        self.profile_menu = None;
        self.file_menu = None;
        self.editor_open = true;
        self.editor_show_advanced = false;
        self.select_profile_for_editor(id, cx);
    }

    fn close_editor(&mut self) {
        self.log_ui_action("close_editor");
        self.editor_open = false;
    }

    fn select_profile(&mut self, id: i64) {
        self.log_ui_action_with_profile("select_profile", id);
        let _ = self.service.select_profile(id);
        self.refresh_cached_profiles_if_needed();
        self.profile_menu = None;
    }

    fn select_profile_for_editor(&mut self, id: i64, cx: &mut Context<Self>) {
        match self.service.select_profile(id) {
            Ok(()) => {
                self.refresh_cached_profiles_if_needed();
                self.editor_mode = ProfileEditorMode::Existing(id);
                if let Some(profile) = self.profiles().into_iter().find(|profile| profile.id == id)
                {
                    self.load_editor_draft(RemoteProfileDraft::from_profile(&profile), cx);
                    self.editor_notice = format!("正在编辑 {}", profile.name);
                }
            }
            Err(error) => {
                self.editor_notice = format!("切换配置失败: {error}");
            }
        }
    }

    fn reset_editor(&mut self, cx: &mut Context<Self>) {
        self.log_ui_action("reset_editor");
        self.refresh_cached_profiles_if_needed();
        match self.editor_mode {
            ProfileEditorMode::Existing(id) => {
                if let Some(profile) = self.profiles().into_iter().find(|profile| profile.id == id)
                {
                    self.load_editor_draft(RemoteProfileDraft::from_profile(&profile), cx);
                    self.editor_notice = format!("已重置为已保存的 {}", profile.name);
                }
            }
            ProfileEditorMode::New => {
                self.load_editor_draft(RemoteProfileDraft::blank(), cx);
                self.editor_notice = String::from("已重置新建表单");
            }
        }
    }

    fn save_editor(&mut self, cx: &mut Context<Self>) {
        self.log_ui_action("save_editor");
        self.ensure_editor_inputs(cx);
        let draft = self.editor_draft(cx);
        let result = match self.editor_mode {
            ProfileEditorMode::Existing(id) => self.service.update_profile(id, draft),
            ProfileEditorMode::New => self.service.create_profile(draft),
        };
        match result {
            Ok(profile) => {
                self.refresh_cached_profiles_if_needed();
                self.editor_mode = ProfileEditorMode::Existing(profile.id);
                self.load_editor_draft(RemoteProfileDraft::from_profile(&profile), cx);
                self.editor_notice = format!("已保存 {}", profile.name);
            }
            Err(error) => {
                self.editor_notice = format!("保存失败: {error}");
            }
        }
    }

    fn test_editor(&mut self, cx: &mut Context<Self>) {
        self.log_ui_action("test_editor");
        self.ensure_editor_inputs(cx);
        match self.service.test_profile_draft(self.editor_draft(cx)) {
            Ok(message) => self.editor_notice = message,
            Err(error) => self.editor_notice = format!("测试连接失败: {error}"),
        }
    }

    fn delete_profile_from_editor(&mut self, id: i64, cx: &mut Context<Self>) {
        self.log_ui_action_with_profile("delete_profile_from_editor", id);
        match self.service.delete_profile(id) {
            Ok(true) => {
                self.refresh_cached_profiles_if_needed();
                if let Some(profile) = self.selected_profile() {
                    self.editor_mode = ProfileEditorMode::Existing(profile.id);
                    self.load_editor_draft(RemoteProfileDraft::from_profile(&profile), cx);
                } else {
                    self.editor_mode = ProfileEditorMode::New;
                    self.load_editor_draft(RemoteProfileDraft::blank(), cx);
                }
                self.editor_notice = String::from("连接配置已删除");
                self.editor_open = false;
            }
            Ok(false) => {
                self.editor_notice = String::from("连接配置不存在");
            }
            Err(error) => {
                self.editor_notice = format!("删除失败: {error}");
            }
        }
    }

    fn duplicate_profile(&mut self, profile: &RemoteProfile, cx: &mut Context<Self>) {
        self.log_ui_action_with_profile("duplicate_profile", profile.id);
        let mut draft = RemoteProfileDraft::from_profile(profile);
        draft.name = format!("{} 副本", profile.name);
        match self.service.create_profile(draft) {
            Ok(profile) => {
                self.refresh_cached_profiles_if_needed();
                self.editor_mode = ProfileEditorMode::Existing(profile.id);
                self.load_editor_draft(RemoteProfileDraft::from_profile(&profile), cx);
                self.editor_notice = format!("已复制 {}", profile.name);
                self.editor_open = true;
            }
            Err(error) => {
                self.editor_notice = format!("复制失败: {error}");
            }
        }
        self.profile_menu = None;
    }

    fn set_editor_protocol(&mut self, protocol: RemoteProtocol, cx: &mut Context<Self>) {
        self.ensure_editor_inputs(cx);
        let old_protocol = self.editor_protocol;
        self.editor_protocol = protocol;
        if let Some(inputs) = self.editor_inputs.as_ref() {
            let current_port = inputs.port.read(cx).text();
            if current_port.trim().is_empty()
                || parse_u16_or_default(&current_port, old_protocol.default_port())
                    == old_protocol.default_port()
            {
                inputs.port.update(cx, |input, input_cx| {
                    input.set_text(protocol.default_port().to_string(), input_cx)
                });
            }
        }
    }

    fn set_editor_auth_method(&mut self, auth_method: AuthMethod) {
        self.editor_auth_method = auth_method;
    }

    fn toggle_editor_passive_mode(&mut self) {
        self.editor_passive_mode = !self.editor_passive_mode;
    }

    fn toggle_editor_jump_enabled(&mut self) {
        self.editor_jump_enabled = !self.editor_jump_enabled;
    }

    fn toggle_editor_pinned(&mut self) {
        self.editor_pinned = !self.editor_pinned;
    }

    fn toggle_editor_show_advanced(&mut self) {
        self.editor_show_advanced = !self.editor_show_advanced;
    }

    fn load_editor_draft(&mut self, draft: RemoteProfileDraft, cx: &mut Context<Self>) {
        self.ensure_editor_inputs(cx);
        self.editor_protocol = draft.protocol;
        self.editor_auth_method = draft.auth_method;
        self.editor_passive_mode = draft.passive_mode;
        self.editor_jump_enabled = draft.jump_enabled;
        self.editor_pinned = draft.pinned;
        if let Some(inputs) = self.editor_inputs.as_ref() {
            inputs.apply_draft(&draft, cx);
        }
    }

    fn editor_draft(&self, cx: &Context<Self>) -> RemoteProfileDraft {
        let Some(inputs) = self.editor_inputs.as_ref() else {
            return RemoteProfileDraft::blank();
        };
        RemoteProfileDraft {
            name: inputs.name.read(cx).text(),
            protocol: self.editor_protocol,
            host: inputs.host.read(cx).text(),
            port: parse_u16_or_default(
                &inputs.port.read(cx).text(),
                self.editor_protocol.default_port(),
            ),
            username: inputs.username.read(cx).text(),
            auth_method: self.editor_auth_method,
            password: inputs.password.read(cx).text(),
            private_key_path: inputs.private_key_path.read(cx).text(),
            private_key_passphrase: inputs.private_key_passphrase.read(cx).text(),
            remote_dir: inputs.remote_dir.read(cx).text(),
            local_dir: inputs.local_dir.read(cx).text(),
            encoding: inputs.encoding.read(cx).text(),
            passive_mode: self.editor_passive_mode,
            connect_timeout_secs: parse_u16_or_default(
                &inputs.connect_timeout_secs.read(cx).text(),
                15,
            ),
            jump_enabled: self.editor_jump_enabled,
            jump_host: inputs.jump_host.read(cx).text(),
            jump_port: parse_u16_or_default(&inputs.jump_port.read(cx).text(), 22),
            jump_username: inputs.jump_username.read(cx).text(),
            jump_password: inputs.jump_password.read(cx).text(),
            jump_private_key_path: inputs.jump_private_key_path.read(cx).text(),
            jump_private_key_passphrase: inputs.jump_private_key_passphrase.read(cx).text(),
            pinned: self.editor_pinned,
            notes: inputs.notes.read(cx).text(),
        }
    }

    fn connect_profile(&mut self, profile_id: i64) {
        self.log_ui_action_with_profile("connect_profile", profile_id);
        let _ = self.service.select_profile(profile_id);
        let svc = Arc::clone(&self.service);
        let _ = svc.connect_profile(profile_id);
        self.profile_menu = None;
    }

    fn disconnect_profile(&mut self, profile_id: i64) {
        self.log_ui_action_with_profile("disconnect_profile", profile_id);
        self.service.disconnect_profile(profile_id);
        self.profile_menu = None;
    }

    fn upload_files(&mut self) {
        self.log_ui_action("upload_files");
        let svc = Arc::clone(&self.service);
        if let Some(paths) = rfd::FileDialog::new().pick_files() {
            let raw = paths
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            let _ = svc.upload_paths(raw);
        }
    }

    fn upload_folder(&mut self) {
        self.log_ui_action("upload_folder");
        let svc = Arc::clone(&self.service);
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            let _ = svc.upload_paths(vec![path.to_string_lossy().into_owned()]);
        }
    }

    fn download_file(&mut self, item: &RemoteFileItem) {
        self.log_ui_action("download_file");
        if item.path.is_empty() {
            return;
        }
        let svc = Arc::clone(&self.service);
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            let local_dir = dir.to_string_lossy().into_owned();
            let _ = svc.start_download(item.path.clone(), &local_dir, item.size);
        }
        self.file_menu = None;
    }

    fn open_text_file(&mut self, item: &RemoteFileItem) {
        self.log_ui_action("open_text_file");
        let svc = Arc::clone(&self.service);
        let _ = svc.open_text_file(item);
        self.file_menu = None;
    }

    fn delete_remote_item(&mut self, item: &RemoteFileItem) {
        self.log_ui_action("delete_remote_item");
        let svc = Arc::clone(&self.service);
        let _ = svc.remote_delete(&item.path, item.is_dir);
        self.file_menu = None;
    }

    fn navigate_dir(&mut self, path: &str) {
        self.log_ui_action("navigate_dir");
        let svc = Arc::clone(&self.service);
        let _ = svc.navigate_dir(path);
        self.file_menu = None;
    }

    fn refresh_dir(&mut self) {
        self.log_ui_action("refresh_dir");
        let svc = Arc::clone(&self.service);
        let _ = svc.refresh_dir();
    }

    fn navigate_up(&mut self) {
        self.log_ui_action("navigate_up");
        let svc = Arc::clone(&self.service);
        let _ = svc.navigate_up();
    }

    fn open_profile_menu(&mut self, profile: RemoteProfile, position: Point<Pixels>) {
        self.file_menu = None;
        self.profile_menu = Some(ProfileMenuState { profile, position });
    }

    fn open_file_menu(&mut self, item: RemoteFileItem, position: Point<Pixels>) {
        self.profile_menu = None;
        self.file_menu = Some(FileMenuState { item, position });
    }

    fn close_menus(&mut self) {
        self.profile_menu = None;
        self.file_menu = None;
    }

    fn open_folder_sheet(&mut self, cx: &mut Context<Self>) {
        self.log_ui_action("open_folder_sheet");
        self.folder_sheet_open = true;
        if let Some(input) = self.new_folder_input.as_ref() {
            set_input_text(input, String::from("新建文件夹"), cx);
        }
    }

    fn close_folder_sheet(&mut self) {
        self.log_ui_action("close_folder_sheet");
        self.folder_sheet_open = false;
    }

    fn create_new_folder(&mut self, cx: &mut Context<Self>) {
        self.log_ui_action("create_new_folder");
        let Some(input) = self.new_folder_input.as_ref() else {
            return;
        };
        let raw = input.read(cx).text();
        let name = raw.trim();
        if name.is_empty() {
            return;
        }
        let base = self.service.current_remote_path();
        if base.is_empty() {
            return;
        }
        let path = if base.ends_with('/') {
            format!("{base}{name}")
        } else {
            format!("{base}/{name}")
        };
        let svc = Arc::clone(&self.service);
        let _ = svc.remote_mkdir(&path);
        self.folder_sheet_open = false;
    }

    fn send_terminal(&mut self, cx: &mut Context<Self>) {
        self.log_ui_action("send_terminal");
        let Some(input) = self.terminal_input.as_ref() else {
            return;
        };
        let mut text = input.read(cx).text();
        if text.trim().is_empty() {
            return;
        }
        if !text.ends_with('\n') {
            text.push('\n');
        }
        let _ = self.service.send_terminal_input(&text);
        input.update(cx, |input, input_cx| input.clear(input_cx));
    }

    fn toggle_transfer_scope(&mut self, show_global: bool) {
        tracing::info!(
            target: "ftp_sftp_ssh.ui",
            action = "toggle_transfer_scope",
            show_global,
            "ftp/sftp/ssh ui action"
        );
        self.show_global_transfers = show_global;
    }

    fn set_transfer_collapsed(&mut self, collapsed: bool) {
        tracing::info!(
            target: "ftp_sftp_ssh.ui",
            action = "set_transfer_collapsed",
            collapsed,
            "ftp/sftp/ssh ui action"
        );
        self.transfer_collapsed = collapsed;
    }

    fn upload_draft(&mut self, draft_id: &str, force: bool) {
        tracing::info!(
            target: "ftp_sftp_ssh.ui",
            action = "upload_draft",
            draft_id,
            force,
            "ftp/sftp/ssh ui action"
        );
        let svc = Arc::clone(&self.service);
        let _ = svc.upload_draft(draft_id, force);
    }

    fn reopen_local_draft(&mut self, draft: &RemoteEditDraft) {
        tracing::info!(
            target: "ftp_sftp_ssh.ui",
            action = "reopen_local_draft",
            path = %draft.local_cache_path,
            "ftp/sftp/ssh ui action"
        );
        let _ = qingqi_platform::shell::open_path(Path::new(&draft.local_cache_path));
    }
}

impl ProfileEditorInputs {
    fn from_draft(cx: &mut App, draft: &RemoteProfileDraft) -> Self {
        Self {
            name: profile_input(cx, "未命名连接", &draft.name, false, 34.0),
            host: profile_input(cx, "example.com / 192.168.1.10", &draft.host, true, 34.0),
            port: profile_input(cx, "22", &draft.port.to_string(), true, 34.0),
            username: profile_input(cx, "用户名", &draft.username, true, 34.0),
            password: profile_input(cx, "密码", &draft.password, true, 34.0),
            private_key_path: profile_input(
                cx,
                "~/.ssh/id_ed25519",
                &draft.private_key_path,
                true,
                34.0,
            ),
            private_key_passphrase: profile_input(
                cx,
                "私钥口令（可选）",
                &draft.private_key_passphrase,
                true,
                34.0,
            ),
            remote_dir: profile_input(cx, "/", &draft.remote_dir, true, 34.0),
            local_dir: profile_input(cx, "~/Downloads", &draft.local_dir, true, 34.0),
            encoding: profile_input(cx, "utf-8", &draft.encoding, true, 34.0),
            connect_timeout_secs: profile_input(
                cx,
                "15",
                &draft.connect_timeout_secs.to_string(),
                true,
                34.0,
            ),
            jump_host: profile_input(cx, "跳板机主机", &draft.jump_host, true, 34.0),
            jump_port: profile_input(cx, "22", &draft.jump_port.to_string(), true, 34.0),
            jump_username: profile_input(cx, "跳板机用户", &draft.jump_username, true, 34.0),
            jump_password: profile_input(cx, "跳板机密码", &draft.jump_password, true, 34.0),
            jump_private_key_path: profile_input(
                cx,
                "跳板机私钥路径",
                &draft.jump_private_key_path,
                true,
                34.0,
            ),
            jump_private_key_passphrase: profile_input(
                cx,
                "跳板机私钥口令",
                &draft.jump_private_key_passphrase,
                true,
                34.0,
            ),
            notes: profile_input(cx, "备注", &draft.notes, false, 78.0),
        }
    }

    fn apply_draft(&self, draft: &RemoteProfileDraft, cx: &mut App) {
        set_input_text(&self.name, draft.name.clone(), cx);
        set_input_text(&self.host, draft.host.clone(), cx);
        set_input_text(&self.port, draft.port.to_string(), cx);
        set_input_text(&self.username, draft.username.clone(), cx);
        set_input_text(&self.password, draft.password.clone(), cx);
        set_input_text(&self.private_key_path, draft.private_key_path.clone(), cx);
        set_input_text(
            &self.private_key_passphrase,
            draft.private_key_passphrase.clone(),
            cx,
        );
        set_input_text(&self.remote_dir, draft.remote_dir.clone(), cx);
        set_input_text(&self.local_dir, draft.local_dir.clone(), cx);
        set_input_text(&self.encoding, draft.encoding.clone(), cx);
        set_input_text(
            &self.connect_timeout_secs,
            draft.connect_timeout_secs.to_string(),
            cx,
        );
        set_input_text(&self.jump_host, draft.jump_host.clone(), cx);
        set_input_text(&self.jump_port, draft.jump_port.to_string(), cx);
        set_input_text(&self.jump_username, draft.jump_username.clone(), cx);
        set_input_text(&self.jump_password, draft.jump_password.clone(), cx);
        set_input_text(
            &self.jump_private_key_path,
            draft.jump_private_key_path.clone(),
            cx,
        );
        set_input_text(
            &self.jump_private_key_passphrase,
            draft.jump_private_key_passphrase.clone(),
            cx,
        );
        set_input_text(&self.notes, draft.notes.clone(), cx);
    }
}

impl Render for FtpSftpSshView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = qingqi_ui::theme_mode::is_dark();
        let accent = theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Cyan);
        let accent_soft = if dark {
            theme::accent_soft_dark(qingqi_plugin::plugin_spec::PluginAccent::Cyan)
        } else {
            theme::accent_soft(qingqi_plugin::plugin_spec::PluginAccent::Cyan)
        };

        self.refresh_cached_profiles_if_needed();
        self.ensure_inputs(cx);
        self.ensure_editor_inputs(cx);

        let entity = cx.entity();

        let profiles = self.filtered_profiles(cx);
        let selected_profile = self.selected_profile();
        let session_summaries = self.service.session_summaries();
        let remote_items = self.service.remote_items();
        let current_path = self.service.current_remote_path();
        let message = self.service.message();
        let status = self.service.status();
        let right_panel_mode = self.service.active_right_panel_mode();
        let terminal_snapshot = self.service.active_terminal_snapshot();
        let protocol_log = self.service.active_protocol_log();
        let drafts = self.service.remote_edit_drafts();
        let current_transfers = self.service.transfer_items();
        let all_transfers = self.service.all_transfer_items();
        let editor = self.editor_snapshot();
        let editor_open = self.editor_open;
        let folder_sheet_open = self.folder_sheet_open;
        let show_global_transfers = self.show_global_transfers;
        let transfer_collapsed = self.transfer_collapsed;
        let search_input = self.search_input.clone();
        let terminal_input = self.terminal_input.clone();
        let new_folder_input = self.new_folder_input.clone();
        let profile_menu = self.profile_menu.clone();
        let file_menu = self.file_menu.clone();

        let summaries_by_id = session_summaries
            .iter()
            .cloned()
            .map(|summary| (summary.profile_id, summary))
            .collect::<HashMap<_, _>>();
        let selected_id = selected_profile.as_ref().map(|profile| profile.id);
        let active_summary = selected_id.and_then(|id| summaries_by_id.get(&id).cloned());
        let selected_protocol = selected_profile
            .as_ref()
            .map(|profile| profile.protocol)
            .unwrap_or(RemoteProtocol::Sftp);
        let supports_files = selected_protocol.supports_file_browser();
        let can_drop = supports_files
            && active_summary
                .as_ref()
                .is_some_and(|summary| summary.status == ConnectionStatus::Connected);

        ui::plugin_surface()
            .font_family(ui::font_ui())
            .relative()
            .on_key_down({
                let entity = entity.clone();
                move |event, window, cx| {
                    if event.keystroke.key == "escape" {
                        tracing::info!(
                            target: "ftp_sftp_ssh.ui",
                            action = "key_escape",
                            "ftp/sftp/ssh ui key event"
                        );
                        entity.update(cx, |view, _cx| {
                            view.close_menus();
                            view.close_editor();
                            view.close_folder_sheet();
                        });
                        window.refresh();
                    }
                }
            })
            .child(
                div()
                    .size_full()
                    .overflow_hidden()
                    .flex()
                    .child(sidebar(
                        dark,
                        accent,
                        profiles,
                        search_input,
                        selected_id,
                        &summaries_by_id,
                        entity.clone(),
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(0.0))
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .child(top_bar(
                                dark,
                                accent,
                                accent_soft,
                                selected_profile.as_ref(),
                                active_summary.as_ref(),
                                status,
                                entity.clone(),
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(0.0))
                                    .min_w(px(0.0))
                                    .flex()
                                    .child(file_workspace(
                                        dark,
                                        accent,
                                        accent_soft,
                                        selected_profile.as_ref(),
                                        active_summary.as_ref(),
                                        remote_items,
                                        current_path,
                                        can_drop,
                                        entity.clone(),
                                    ))
                                    .child(protocol_panel(
                                        dark,
                                        selected_profile.as_ref(),
                                        active_summary.as_ref(),
                                        right_panel_mode,
                                        terminal_snapshot,
                                        protocol_log,
                                        terminal_input,
                                        entity.clone(),
                                    )),
                            )
                            .child(transfer_panel(
                                dark,
                                accent,
                                drafts,
                                current_transfers,
                                all_transfers,
                                show_global_transfers,
                                transfer_collapsed,
                                entity.clone(),
                            ))
                            .child(status_bar(dark, accent, message)),
                    ),
            )
            .when(editor_open, |root| {
                root.child(profile_editor_overlay(
                    dark,
                    editor,
                    selected_profile,
                    entity.clone(),
                ))
            })
            .when(folder_sheet_open, |root| {
                root.child(new_folder_overlay(dark, new_folder_input, entity.clone()))
            })
            .when(profile_menu.is_some(), |root| {
                root.child(profile_menu_overlay(
                    dark,
                    profile_menu,
                    &summaries_by_id,
                    entity.clone(),
                ))
            })
            .when(file_menu.is_some(), |root| {
                root.child(file_menu_overlay(dark, file_menu, entity.clone()))
            })
    }
}

fn top_bar(
    dark: bool,
    _accent: gpui::Rgba,
    _accent_soft: gpui::Rgba,
    selected: Option<&RemoteProfile>,
    summary: Option<&SessionSummary>,
    status: ConnectionStatus,
    panel: Entity<FtpSftpSshView>,
) -> impl IntoElement {
    let status_color = match status {
        ConnectionStatus::Connected => theme::semantic().success,
        ConnectionStatus::Failed => theme::semantic().danger,
        ConnectionStatus::Idle => ui::text_tertiary(),
    };
    let title = selected
        .map(|profile| profile.name.clone())
        .unwrap_or_else(|| String::from("未选择连接"));
    let detail = selected
        .map(|profile| profile.endpoint())
        .unwrap_or_else(|| String::from("SFTP / FTP / FTPS · 未连接"));
    let is_connected = summary.is_some_and(|summary| summary.status == ConnectionStatus::Connected);
    let selected_id = selected.map(|profile| profile.id);

    div()
        .h(px(38.0))
        .px(px(10.0))
        .border_b_1()
        .border_color(ui::border_light())
        .flex()
        .items_center()
        .gap(px(6.0))
        .bg(theme::semantic().bg_surface)
        .child(div().size(px(7.0)).rounded(px(999.0)).bg(status_color))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_clamp(1)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(ui::text_secondary())
                        .line_clamp(1)
                        .child(detail),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(ui::text_secondary())
                        .child("·"),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(status_color)
                        .line_clamp(1)
                        .child(status.label()),
                ),
        )
        .child(div().flex_1())
        .child(
            frost_button(
                if is_connected { "已连接" } else { "连接" },
                Some("↗"),
                if is_connected { "ghost" } else { "primary" },
                dark,
                false,
            )
            .id("ftp-connect-selected")
            .when(selected_id.is_none() || is_connected, |button| {
                button.opacity(0.58)
            })
            .on_click({
                let panel = panel.clone();
                move |_, window, cx| {
                    if let Some(profile_id) = selected_id {
                        cx.update_entity(&panel, |view, _cx| view.connect_profile(profile_id));
                    }
                    window.refresh();
                }
            }),
        )
        .child(
            frost_button("刷新", Some("⟳"), "ghost", dark, false)
                .id("ftp-refresh")
                .when(!is_connected, |button| button.opacity(0.42))
                .on_click({
                    let panel = panel.clone();
                    move |_, window, cx| {
                        cx.update_entity(&panel, |view, _cx| view.refresh_dir());
                        window.refresh();
                    }
                }),
        )
        .child(
            frost_button("断开", Some("⏻"), "ghost", dark, true)
                .id("ftp-disconnect-selected")
                .when(!is_connected, |button| button.opacity(0.42))
                .on_click({
                    let panel = panel.clone();
                    move |_, window, cx| {
                        if let Some(profile_id) = selected_id {
                            cx.update_entity(&panel, |view, _cx| {
                                view.disconnect_profile(profile_id)
                            });
                        }
                        window.refresh();
                    }
                }),
        )
}

fn sidebar(
    dark: bool,
    accent: gpui::Rgba,
    profiles: Vec<RemoteProfile>,
    search_input: Option<Entity<TextInput>>,
    selected_id: Option<i64>,
    summaries_by_id: &HashMap<i64, SessionSummary>,
    panel: Entity<FtpSftpSshView>,
) -> impl IntoElement {
    let open_count = profiles.len();
    div()
        .w(px(220.0))
        .min_h(px(0.0))
        .border_r_1()
        .border_color(ui::border_light())
        .bg(ui::bg_keycap())
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.0))
                .pt(px(8.0))
                .pb(px(6.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("连接管理"),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(ui::text_secondary())
                                        .child(format!("{open_count} 个")),
                                ),
                        )
                        .child(
                            div()
                                .size(px(26.0))
                                .rounded(px(6.0))
                                .bg(accent)
                                .text_size(px(16.0))
                                .text_color(theme::white())
                                .flex()
                                .items_center()
                                .justify_center()
                                .hover(move |style| style.cursor_pointer())
                                .child("+")
                                .id("ftp-open-new-profile-sidebar")
                                .on_click({
                                    let panel = panel.clone();
                                    move |_, window, cx| {
                                        cx.update_entity(&panel, |view, _cx| {
                                            view.begin_new_profile(_cx)
                                        });
                                        window.refresh();
                                    }
                                }),
                        ),
                )
                .child(search_input_shell(search_input, dark)),
        )
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .id("ftp-scroll")
                .overflow_y_scroll()
                .px(px(6.0))
                .py(px(4.0))
                .flex()
                .flex_col()
                .gap(px(3.0))
                .when(profiles.is_empty(), |list| {
                    list.child(
                        div()
                            .h(px(140.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(11.0))
                            .text_color(ui::text_secondary())
                            .child("暂无连接"),
                    )
                })
                .children(profiles.into_iter().map(|profile| {
                    let summary = summaries_by_id.get(&profile.id).cloned();
                    let selected = selected_id == Some(profile.id);
                    connection_card(dark, profile, summary, selected, panel.clone())
                })),
        )
        .child(
            div()
                .h(px(26.0))
                .px(px(10.0))
                .text_size(px(10.0))
                .text_color(ui::text_tertiary())
                .flex()
                .items_center()
                .child(format!("{open_count} 个连接 · 右键查看操作")),
        )
}

fn connection_card(
    dark: bool,
    profile: RemoteProfile,
    summary: Option<SessionSummary>,
    selected: bool,
    panel: Entity<FtpSftpSshView>,
) -> impl IntoElement {
    let is_connected = summary
        .as_ref()
        .is_some_and(|summary| summary.status == ConnectionStatus::Connected);
    let profile_id = profile.id;

    div()
        .id(("ftp-profile-row", profile_id as u64))
        .w_full()
        .h(px(36.0))
        .rounded(px(6.0))
        .bg(theme::rgba_with_alpha(
            theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Cyan),
            if selected { 1.0 } else { 0.0 },
        ))
        .when(!selected, |row| {
            row.hover(move |style| style.bg(row_hover_color(dark)).cursor_pointer())
        })
        .when(selected, |row| row.hover(|style| style.cursor_pointer()))
        .on_click({
            let panel = panel.clone();
            move |event, window, cx| {
                cx.update_entity(&panel, |view, _cx| {
                    view.select_profile(profile_id);
                    if event.click_count() >= 2 && !event.is_right_click() {
                        view.connect_profile(profile_id);
                    }
                });
                window.refresh();
            }
        })
        .on_mouse_down(MouseButton::Right, {
            let panel = panel.clone();
            let profile = profile.clone();
            move |event, window, cx| {
                cx.update_entity(&panel, |view, _cx| {
                    view.open_profile_menu(profile.clone(), event.position)
                });
                cx.stop_propagation();
                window.refresh();
            }
        })
        .px(px(6.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(div().size(px(6.0)).rounded(px(999.0)).bg(if is_connected {
            theme::semantic().success
        } else {
            ui::text_tertiary()
        }))
        .child(
            div()
                .size(px(20.0))
                .rounded(px(4.0))
                .bg(if selected {
                    hsla(0.0, 0.0, 1.0, 0.18)
                } else if dark {
                    hsla(0.0, 0.0, 1.0, 0.05)
                } else {
                    hsla(0.56, 0.80, 0.88, 0.22)
                })
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.0))
                .text_color(if selected {
                    theme::white()
                } else {
                    theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Cyan)
                })
                .child("▣"),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if selected {
                            theme::white()
                        } else {
                            theme::semantic().text_primary
                        })
                        .line_clamp(1)
                        .child(profile.name.clone()),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(if selected {
                            hsla(0.0, 0.0, 1.0, 0.78).into()
                        } else {
                            ui::text_secondary()
                        })
                        .line_clamp(1)
                        .child(profile.endpoint()),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .h(px(16.0))
                        .px(px(5.0))
                        .rounded(px(4.0))
                        .bg(if selected {
                            theme::rgba_with_alpha(theme::white(), 0.18)
                        } else {
                            theme::rgba_with_alpha(theme::semantic().bg_surface, 1.0)
                        })
                        .text_size(px(8.0))
                        .text_color(if selected {
                            theme::white()
                        } else {
                            theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Cyan)
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(profile.protocol.label()),
                )
                .when(profile.pinned, |row| {
                    row.child(
                        div()
                            .text_size(px(10.0))
                            .text_color(if selected {
                                theme::white()
                            } else {
                                theme::semantic().warning
                            })
                            .child("★"),
                    )
                })
                .child(
                    div()
                        .id("ftp-profile-menu-trigger")
                        .text_size(px(10.0))
                        .text_color(if selected {
                            theme::white()
                        } else {
                            ui::text_tertiary()
                        })
                        .hover(move |style| style.cursor_pointer())
                        .child("⋯")
                        .on_click({
                            let panel = panel.clone();
                            let profile = profile.clone();
                            move |event, window, cx| {
                                cx.stop_propagation();
                                cx.update_entity(&panel, |view, _cx| {
                                    view.open_profile_menu(profile.clone(), event.position())
                                });
                                window.refresh();
                            }
                        }),
                ),
        )
}

fn file_workspace(
    dark: bool,
    accent: gpui::Rgba,
    _accent_soft: gpui::Rgba,
    selected: Option<&RemoteProfile>,
    summary: Option<&SessionSummary>,
    remote_items: Vec<RemoteFileItem>,
    current_path: String,
    can_drop: bool,
    panel: Entity<FtpSftpSshView>,
) -> impl IntoElement {
    let supports_files = selected.is_some_and(|profile| profile.protocol.supports_file_browser());
    let path_text = if current_path.is_empty() {
        selected
            .map(|profile| profile.remote_dir.clone())
            .unwrap_or_else(|| String::from("/"))
    } else {
        current_path
    };
    let upload_hint = if can_drop {
        format!("拖拽文件或目录上传到 {path_text}")
    } else if selected.is_none() {
        String::from("先从左侧选择一个连接")
    } else if !supports_files {
        String::from("当前协议没有文件浏览能力")
    } else {
        String::from("连接后才能拖拽上传")
    };
    let connected = summary.is_some_and(|summary| summary.status == ConnectionStatus::Connected);
    let item_count = remote_items
        .iter()
        .filter(|item| !item.path.is_empty())
        .count();

    div()
        .w(px(440.0))
        .min_w(px(300.0))
        .min_h(px(0.0))
        .border_r_1()
        .border_color(ui::border_light())
        .bg(theme::semantic().bg_surface)
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(38.0))
                .px(px(10.0))
                .border_b_1()
                .border_color(ui::border_light())
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(theme::accent_color(
                                    qingqi_plugin::plugin_spec::PluginAccent::Cyan,
                                ))
                                .child(Icon::new(IconName::FolderClosed).xsmall().text_color(
                                    theme::accent_color(
                                        qingqi_plugin::plugin_spec::PluginAccent::Cyan,
                                    ),
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("远程目录"),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(ui::text_secondary())
                        .child(format!("{item_count} 项")),
                ),
        )
        .child(
            div()
                .h(px(32.0))
                .px(px(10.0))
                .border_b_1()
                .border_color(ui::border_light())
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    small_icon_action(IconName::ArrowUp, dark)
                        .id("ftp-navigate-up")
                        .when(!connected || !supports_files, |button| button.opacity(0.42))
                        .on_click({
                            let panel = panel.clone();
                            move |_, window, cx| {
                                cx.update_entity(&panel, |view, _cx| view.navigate_up());
                                window.refresh();
                            }
                        }),
                )
                .child(
                    small_icon_action(IconName::FolderOpen, dark)
                        .id("ftp-home-dir")
                        .when(!connected || !supports_files, |button| button.opacity(0.42))
                        .on_click({
                            let panel = panel.clone();
                            let home_path = selected.map(|profile| profile.remote_dir.clone());
                            move |_, window, cx| {
                                if let Some(path) = home_path.as_ref() {
                                    cx.update_entity(&panel, |view, _cx| view.navigate_dir(path));
                                }
                                window.refresh();
                            }
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .font_family("SF Mono")
                        .text_size(px(11.0))
                        .line_clamp(1)
                        .child(path_text.clone()),
                )
                .child(
                    small_icon_action(IconName::Redo2, dark)
                        .id("ftp-refresh-inline")
                        .when(!connected || !supports_files, |button| button.opacity(0.42))
                        .on_click({
                            let panel = panel.clone();
                            move |_, window, cx| {
                                cx.update_entity(&panel, |view, _cx| view.refresh_dir());
                                window.refresh();
                            }
                        }),
                )
                .child(
                    small_icon_action(IconName::ArrowUp, dark)
                        .id("ftp-upload-files")
                        .when(!connected || !supports_files, |button| button.opacity(0.42))
                        .on_click({
                            let panel = panel.clone();
                            move |_, window, cx| {
                                cx.update_entity(&panel, |view, _cx| view.upload_files());
                                window.refresh();
                            }
                        }),
                )
                .child(
                    small_icon_action(IconName::FolderClosed, dark)
                        .id("ftp-upload-folder")
                        .when(!connected || !supports_files, |button| button.opacity(0.42))
                        .on_click({
                            let panel = panel.clone();
                            move |_, window, cx| {
                                cx.update_entity(&panel, |view, _cx| view.upload_folder());
                                window.refresh();
                            }
                        }),
                )
                .child(
                    small_icon_action(IconName::Plus, dark)
                        .id("ftp-create-folder")
                        .when(!connected || !supports_files, |button| button.opacity(0.42))
                        .on_click({
                            let panel = panel.clone();
                            move |_, window, cx| {
                                cx.update_entity(&panel, |view, _cx| view.open_folder_sheet(_cx));
                                window.refresh();
                            }
                        }),
                ),
        )
        .child(
            div()
                .h(px(26.0))
                .px(px(10.0))
                .border_b_1()
                .border_color(ui::border_light())
                .bg(ui::bg_keycap())
                .flex()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(px(10.0))
                        .text_color(ui::text_secondary())
                        .child("名称"),
                )
                .child(
                    div()
                        .w(px(64.0))
                        .text_size(px(10.0))
                        .text_color(ui::text_secondary())
                        .child("大小"),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .id("ftp-remote-file-zone")
                .can_drop(move |_, _, _| can_drop)
                .drag_over::<ExternalPaths>(move |style, _, _, _| {
                    style.bg(theme::rgba_with_alpha(
                        theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Cyan),
                        if dark { 0.14 } else { 0.10 },
                    ))
                })
                .on_drop({
                    let panel = panel.clone();
                    move |paths: &ExternalPaths, window, cx| {
                        let raw = paths
                            .paths()
                            .iter()
                            .map(|path| path.to_string_lossy().into_owned())
                            .collect::<Vec<_>>();
                        let svc = Arc::clone(&panel.read(cx).service);
                        let _ = svc.upload_paths(raw);
                        window.refresh();
                    }
                })
                .id("ftp-scroll")
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .when(remote_items.is_empty(), |list| {
                    list.child(
                        div()
                            .flex_1()
                            .min_h(px(180.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(11.0))
                            .text_color(ui::text_secondary())
                            .child(upload_hint.clone()),
                    )
                })
                .children(
                    remote_items
                        .into_iter()
                        .map(|item| remote_entry_row(dark, item, panel.clone())),
                ),
        )
        .child(
            div()
                .h(px(22.0))
                .px(px(10.0))
                .border_t_1()
                .border_color(ui::border_light())
                .text_size(px(10.0))
                .text_color(if can_drop {
                    accent
                } else {
                    ui::text_secondary()
                })
                .flex()
                .items_center()
                .line_clamp(1)
                .child(upload_hint),
        )
}

fn remote_entry_row(
    dark: bool,
    item: RemoteFileItem,
    panel: Entity<FtpSftpSshView>,
) -> impl IntoElement {
    let is_text_file = !item.is_dir && looks_like_text_file_name(&item.name);
    let primary_label = if item.is_dir { "进入" } else { "下载" };
    let item_for_primary = item.clone();
    let item_for_text = item.clone();
    let item_for_menu = item.clone();

    div()
        .id(SharedString::from(format!(
            "ftp-item-{}",
            item.name.replace('/', "_")
        )))
        .h(px(30.0))
        .border_b_1()
        .border_color(ui::border_light())
        .bg(if item.path.is_empty() {
            theme::rgba_with_alpha(ui::row_hover(), 1.0)
        } else if item.selected {
            if dark {
                theme::rgba_with_alpha(
                    theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Cyan),
                    0.24,
                )
            } else {
                theme::rgba_with_alpha(
                    theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Cyan),
                    0.16,
                )
            }
        } else {
            theme::rgba_with_alpha(theme::semantic().bg_surface, 1.0)
        })
        .hover(move |style| style.bg(row_hover_color(dark)).cursor_pointer())
        .on_mouse_down(MouseButton::Right, {
            let panel = panel.clone();
            let item = item.clone();
            move |event, window, cx| {
                cx.update_entity(&panel, |view, _cx| {
                    view.open_file_menu(item.clone(), event.position)
                });
                cx.stop_propagation();
                window.refresh();
            }
        })
        .px(px(10.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .w(px(16.0))
                .flex()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.0))
                .text_color(if item.is_dir {
                    theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Cyan)
                } else {
                    ui::text_secondary()
                })
                .child(
                    Icon::new(if item.is_dir {
                        IconName::FolderClosed
                    } else {
                        IconName::File
                    })
                    .xsmall()
                    .text_color(if item.is_dir {
                        theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Cyan)
                    } else {
                        ui::text_secondary()
                    }),
                ),
        )
        .child(
            div().flex_1().min_w(px(0.0)).flex().items_center().child(
                div()
                    .text_size(px(11.0))
                    .line_clamp(1)
                    .child(item.name.clone()),
            ),
        )
        .child(
            div()
                .w(px(64.0))
                .text_size(px(10.0))
                .text_color(ui::text_secondary())
                .items_center()
                .child(if item.is_dir {
                    String::new()
                } else {
                    item.meta.clone()
                }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .when(!item.path.is_empty(), |row| {
                    row.child(
                        small_action(primary_label, dark)
                            .id("ftp-remote-primary-action")
                            .on_click({
                                let panel = panel.clone();
                                move |_, window, cx| {
                                    if item_for_primary.is_dir {
                                        cx.update_entity(&panel, |view, _cx| {
                                            view.navigate_dir(&item_for_primary.path)
                                        });
                                    } else {
                                        cx.update_entity(&panel, |view, _cx| {
                                            view.download_file(&item_for_primary)
                                        });
                                    }
                                    window.refresh();
                                }
                            }),
                    )
                })
                .when(is_text_file, |row| {
                    row.child(
                        small_action("文本", dark)
                            .id("ftp-remote-open-text")
                            .on_click({
                                let panel = panel.clone();
                                move |_, window, cx| {
                                    cx.update_entity(&panel, |view, _cx| {
                                        view.open_text_file(&item_for_text)
                                    });
                                    window.refresh();
                                }
                            }),
                    )
                })
                .when(!item.path.is_empty(), |row| {
                    row.child(
                        small_icon_action(IconName::Ellipsis, dark)
                            .id("ftp-remote-menu")
                            .on_click({
                                let panel = panel.clone();
                                move |event, window, cx| {
                                    cx.update_entity(&panel, |view, _cx| {
                                        view.open_file_menu(item_for_menu.clone(), event.position())
                                    });
                                    window.refresh();
                                }
                            }),
                    )
                }),
        )
}

fn protocol_panel(
    dark: bool,
    selected: Option<&RemoteProfile>,
    summary: Option<&SessionSummary>,
    right_panel_mode: RightPanelMode,
    terminal_snapshot: TerminalSnapshot,
    protocol_log: Vec<ProtocolLogEntry>,
    terminal_input: Option<Entity<TextInput>>,
    panel: Entity<FtpSftpSshView>,
) -> impl IntoElement {
    let title = match right_panel_mode {
        RightPanelMode::Terminal => "SSH 终端",
        RightPanelMode::FtpLog => "FTP 命令日志",
        RightPanelMode::Empty => "协议工作区",
    };
    let protocol = selected.map(|profile| profile.protocol);

    div()
        .flex_1()
        .min_w(px(360.0))
        .min_h(px(0.0))
        .bg(theme::semantic().bg_surface)
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(38.0))
                .px(px(10.0))
                .border_b_1()
                .border_color(ui::border_light())
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div().flex().flex_col().gap(px(1.0)).child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    ),
                )
                .child(meta_badge(
                    summary
                        .map(|summary| summary.status.label().to_string())
                        .unwrap_or_else(|| String::from("未连接")),
                    ui::text_secondary(),
                    dark,
                )),
        )
        .child(match right_panel_mode {
            RightPanelMode::Terminal => terminal_workspace(
                dark,
                protocol,
                summary,
                terminal_snapshot,
                terminal_input,
                panel.clone(),
            )
            .into_any_element(),
            RightPanelMode::FtpLog => {
                ftp_log_workspace(dark, protocol_log, summary, panel.clone()).into_any_element()
            }
            RightPanelMode::Empty => empty_protocol_workspace(dark, protocol).into_any_element(),
        })
}

fn terminal_workspace(
    dark: bool,
    protocol: Option<RemoteProtocol>,
    summary: Option<&SessionSummary>,
    terminal_snapshot: TerminalSnapshot,
    terminal_input: Option<Entity<TextInput>>,
    panel: Entity<FtpSftpSshView>,
) -> impl IntoElement {
    let connected = summary.is_some_and(|summary| summary.status == ConnectionStatus::Connected);
    let terminal_bg = ui::terminal_bg();
    let terminal_fg = ui::terminal_fg();
    let terminal_muted = ui::terminal_muted();
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.0))
                .h(px(30.0))
                .border_b_1()
                .border_color(ui::border_light())
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    Icon::new(IconName::SquareTerminal)
                        .xsmall()
                        .text_color(if connected {
                            theme::semantic().info
                        } else {
                            ui::text_secondary()
                        }),
                )
                .child(status_pill(
                    terminal_snapshot.status.label().to_string(),
                    if connected {
                        theme::semantic().info
                    } else {
                        ui::text_tertiary()
                    },
                    dark,
                ))
                .when(!terminal_snapshot.cwd_hint.is_empty(), |row| {
                    row.child(
                        div()
                            .rounded(px(999.0))
                            .bg(ui::bg_keycap())
                            .px(px(6.0))
                            .py(px(3.0))
                            .font_family("SF Mono")
                            .text_size(px(9.0))
                            .text_color(ui::text_secondary())
                            .line_clamp(1)
                            .child(terminal_snapshot.cwd_hint.clone()),
                    )
                })
                .child(div().flex_1())
                .child(
                    frost_button("打开终端", None, "ghost", dark, false)
                        .id("ftp-open-terminal")
                        .when(!connected, |button| button.opacity(0.42))
                        .on_click({
                            let panel = panel.clone();
                            move |_, window, cx| {
                                let _ = panel.read(cx).service.open_terminal();
                                window.refresh();
                            }
                        }),
                )
                .child(
                    frost_button("同步目录", None, "ghost", dark, false)
                        .id("ftp-sync-terminal-dir")
                        .when(
                            !connected || protocol != Some(RemoteProtocol::Sftp),
                            |button| button.opacity(0.42),
                        )
                        .on_click({
                            let panel = panel.clone();
                            move |_, window, cx| {
                                let _ = panel.read(cx).service.sync_terminal_to_current_dir();
                                window.refresh();
                            }
                        }),
                )
                .child(
                    frost_button("关闭", None, "ghost", dark, false)
                        .id("ftp-close-terminal")
                        .when(!connected, |button| button.opacity(0.42))
                        .on_click({
                            let panel = panel.clone();
                            move |_, window, cx| {
                                panel.read(cx).service.close_terminal();
                                window.refresh();
                            }
                        }),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .p(px(6.0))
                .bg(theme::semantic().bg_subtle_2)
                .child(
                    div()
                        .id("ftp-scroll")
                        .size_full()
                        .overflow_y_scroll()
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(ui::terminal_border())
                        .bg(terminal_bg)
                        .px(px(8.0))
                        .py(px(6.0))
                        .font_family("SF Mono")
                        .text_size(px(11.0))
                        .line_height(px(15.0))
                        .text_color(terminal_fg)
                        .when(terminal_snapshot.lines.is_empty(), |list| {
                            list.child(
                                div()
                                    .text_size(px(10.0))
                                    .line_height(px(15.0))
                                    .text_color(terminal_muted)
                                    .child("terminal ready; output will appear here"),
                            )
                        })
                        .children(terminal_snapshot.lines.into_iter().map(|line| {
                            div()
                                .min_w(px(0.0))
                                .text_color(terminal_fg)
                                .child(line.replace('\t', "    "))
                        })),
                ),
        )
        .child(
            div()
                .px(px(6.0))
                .py(px(4.0))
                .border_t_1()
                .border_color(ui::border_light())
                .flex()
                .items_center()
                .gap(px(4.0))
                .bg(theme::semantic().bg_surface)
                .child(
                    div()
                        .font_family("SF Mono")
                        .text_size(px(11.0))
                        .text_color(ui::text_secondary())
                        .child("$"),
                )
                .child(
                    div()
                        .flex_1()
                        .rounded(px(5.0))
                        .border_1()
                        .border_color(ui::border_light())
                        .bg(ui::bg_keycap())
                        .child(
                            terminal_input
                                .unwrap_or_else(|| panic!("terminal input should be initialized")),
                        ),
                )
                .child(
                    frost_button("发送", Some("↵"), "primary", dark, false)
                        .id("ftp-send-terminal")
                        .when(!connected, |button| button.opacity(0.42))
                        .on_click({
                            let panel = panel.clone();
                            move |_, window, cx| {
                                cx.update_entity(&panel, |view, _cx| view.send_terminal(_cx));
                                window.refresh();
                            }
                        }),
                ),
        )
}

fn ftp_log_workspace(
    dark: bool,
    protocol_log: Vec<ProtocolLogEntry>,
    summary: Option<&SessionSummary>,
    panel: Entity<FtpSftpSshView>,
) -> impl IntoElement {
    let count = protocol_log.len();
    let profile_id = summary.map(|summary| summary.profile_id);
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.0))
                .h(px(30.0))
                .border_b_1()
                .border_color(ui::border_light())
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(meta_badge(
                    format!("{count} 条"),
                    theme::semantic().success,
                    dark,
                ))
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(ui::text_secondary())
                        .child("命令蓝 / 响应绿 / 信息灰 / 错误红"),
                )
                .child(div().flex_1())
                .child(
                    frost_button("清空", None, "ghost", dark, false)
                        .id("ftp-clear-log")
                        .when(profile_id.is_none(), |button| button.opacity(0.42))
                        .on_click({
                            let panel = panel.clone();
                            move |_, window, cx| {
                                if let Some(profile_id) = profile_id {
                                    panel.read(cx).service.clear_protocol_log(profile_id);
                                }
                                window.refresh();
                            }
                        }),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .id("ftp-scroll")
                .overflow_y_scroll()
                .px(px(10.0))
                .py(px(8.0))
                .flex()
                .flex_col()
                .gap(px(4.0))
                .when(protocol_log.is_empty(), |list| {
                    list.child(empty_state_card(
                        dark,
                        "当前没有 FTP 日志",
                        "FTP / FTPS session 会把命令和响应写到这里，方便排障",
                    ))
                })
                .children(
                    protocol_log
                        .into_iter()
                        .map(|entry| protocol_log_row(dark, entry)),
                ),
        )
}

fn empty_protocol_workspace(dark: bool, protocol: Option<RemoteProtocol>) -> impl IntoElement {
    let message = match protocol {
        Some(RemoteProtocol::Ftp) | Some(RemoteProtocol::Ftps) => "连接后这里会显示 FTP 命令日志",
        Some(RemoteProtocol::Sftp) | Some(RemoteProtocol::Ssh) => "连接后这里会显示 SSH 终端",
        None => "选择一个连接后，这里会按协议显示终端或日志",
    };
    div()
        .flex_1()
        .min_h(px(0.0))
        .p(px(10.0))
        .child(empty_state_card(dark, "协议工作区未激活", message))
}

fn protocol_log_row(_dark: bool, entry: ProtocolLogEntry) -> impl IntoElement {
    let color = match entry.kind {
        ProtocolLogKind::Command => theme::semantic().info,
        ProtocolLogKind::Response => theme::semantic().success,
        ProtocolLogKind::Info => ui::text_secondary(),
        ProtocolLogKind::Error => theme::semantic().danger,
    };

    div()
        .rounded(px(6.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(ui::bg_keycap())
        .px(px(8.0))
        .py(px(6.0))
        .font_family("SF Mono")
        .text_size(px(10.0))
        .line_height(px(15.0))
        .child(div().text_color(color).child(entry.display_text()))
}

fn row_hover_color(dark: bool) -> gpui::Hsla {
    theme::rgba_with_alpha(theme::semantic().row_hover, if dark { 0.86 } else { 0.72 })
}

fn transfer_panel(
    dark: bool,
    accent: gpui::Rgba,
    drafts: Vec<RemoteEditDraft>,
    current_transfers: Vec<TransferItem>,
    all_transfers: Vec<SessionTransferItem>,
    show_global_transfers: bool,
    collapsed: bool,
    panel: Entity<FtpSftpSshView>,
) -> impl IntoElement {
    let transfer_scope_items = if show_global_transfers {
        None
    } else {
        Some(current_transfers.clone())
    };
    let transfer_counts_snapshot = if let Some(items) = transfer_scope_items.as_ref() {
        transfer_counts(items)
    } else {
        let all = all_transfers
            .iter()
            .map(|session_item| session_item.item.clone())
            .collect::<Vec<_>>();
        transfer_counts(&all)
    };
    let has_content =
        !drafts.is_empty() || !current_transfers.is_empty() || !all_transfers.is_empty();

    div()
        .border_t_1()
        .border_color(ui::border_light())
        .h(if collapsed { px(28.0) } else { px(150.0) })
        .bg(theme::semantic().bg_surface)
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(28.0))
                .px(px(10.0))
                .border_b_1()
                .border_color(ui::border_light())
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            small_icon_action(
                                if collapsed {
                                    IconName::ChevronUp
                                } else {
                                    IconName::ChevronDown
                                },
                                dark,
                            )
                            .id("ftp-toggle-transfer-panel")
                            .on_click({
                                let panel = panel.clone();
                                move |_, window, cx| {
                                    cx.update_entity(&panel, |view, _cx| {
                                        view.set_transfer_collapsed(!collapsed)
                                    });
                                    window.refresh();
                                }
                            }),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("传输记录"),
                        )
                        .child(transfer_count_chip(
                            "总数",
                            transfer_counts_snapshot.total,
                            accent,
                            dark,
                            transfer_counts_snapshot.total > 0,
                        ))
                        .when(transfer_counts_snapshot.active > 0, |row| {
                            row.child(transfer_count_chip(
                                "活跃",
                                transfer_counts_snapshot.active,
                                theme::semantic().info,
                                dark,
                                true,
                            ))
                        })
                        .when(collapsed && !has_content, |row| {
                            row.child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(ui::text_secondary())
                                    .child("暂无任务"),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .when(collapsed, |row| {
                            row.child(
                                small_icon_text_action(IconName::ChevronUp, "展开", dark)
                                    .id("ftp-expand-transfer-panel")
                                    .on_click({
                                        let panel = panel.clone();
                                        move |_, window, cx| {
                                            cx.update_entity(&panel, |view, _cx| {
                                                view.set_transfer_collapsed(false)
                                            });
                                            window.refresh();
                                        }
                                    }),
                            )
                        })
                        .when(!collapsed, |row| {
                            row.child(
                                small_icon_text_action(IconName::Close, "关闭", dark)
                                    .id("ftp-collapse-transfer-panel")
                                    .on_click({
                                        let panel = panel.clone();
                                        move |_, window, cx| {
                                            cx.update_entity(&panel, |view, _cx| {
                                                view.set_transfer_collapsed(true)
                                            });
                                            window.refresh();
                                        }
                                    }),
                            )
                        })
                        .child(
                            segmented_chip("当前 session", !show_global_transfers, dark)
                                .id("ftp-scope-current")
                                .on_click({
                                    let panel = panel.clone();
                                    move |_, window, cx| {
                                        cx.update_entity(&panel, |view, _cx| {
                                            view.toggle_transfer_scope(false)
                                        });
                                        window.refresh();
                                    }
                                }),
                        )
                        .child(
                            segmented_chip("全部 session", show_global_transfers, dark)
                                .id("ftp-scope-all")
                                .on_click({
                                    let panel = panel.clone();
                                    move |_, window, cx| {
                                        cx.update_entity(&panel, |view, _cx| {
                                            view.toggle_transfer_scope(true)
                                        });
                                        window.refresh();
                                    }
                                }),
                        )
                        .child(
                            frost_button("清空已完成", None, "ghost", dark, false)
                                .id("ftp-clear-finished-transfers")
                                .when(show_global_transfers, |button| button.opacity(0.42))
                                .on_click({
                                    let panel = panel.clone();
                                    move |_, window, cx| {
                                        if !show_global_transfers {
                                            panel.read(cx).service.clear_finished_transfers();
                                        }
                                        window.refresh();
                                    }
                                }),
                        ),
                ),
        )
        .when(!collapsed, |panel_root| {
            panel_root.child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .id("ftp-scroll")
                    .overflow_y_scroll()
                    .px(px(8.0))
                    .py(px(6.0))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .when(!drafts.is_empty(), |list| {
                        list.child(draft_section(dark, drafts.clone(), panel.clone()))
                    })
                    .when(
                        current_transfers.is_empty()
                            && all_transfers.is_empty()
                            && drafts.is_empty(),
                        |list| {
                            list.child(empty_state_card(
                                dark,
                                "还没有传输或待回传草稿",
                                "上传、下载或打开文本文件后，这里会显示记录",
                            ))
                        },
                    )
                    .when(!show_global_transfers, |list| {
                        list.children(
                            current_transfers
                                .iter()
                                .map(|item| transfer_card(dark, item, None, panel.clone())),
                        )
                    })
                    .when(show_global_transfers, |list| {
                        list.children(all_transfers.iter().map(|item| {
                            transfer_card(
                                dark,
                                &item.item,
                                Some(item.session_name.clone()),
                                panel.clone(),
                            )
                        }))
                    }),
            )
        })
}

fn draft_section(
    dark: bool,
    drafts: Vec<RemoteEditDraft>,
    panel: Entity<FtpSftpSshView>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(10.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child("待回传文本草稿"),
        )
        .children(drafts.into_iter().map(|draft| {
            let state_color = match draft.state {
                RemoteEditState::Synced => theme::semantic().success,
                RemoteEditState::ModifiedLocal => theme::semantic().warning,
                RemoteEditState::UploadingBack => theme::semantic().info,
                RemoteEditState::ConflictRisk => theme::semantic().danger,
                RemoteEditState::UploadFailed => theme::semantic().danger,
            };
            let draft_for_open = draft.clone();
            let draft_for_upload = draft.clone();
            div()
                .rounded(px(8.0))
                .border_1()
                .border_color(ui::border_light())
                .bg(ui::bg_keycap())
                .p(px(8.0))
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(1.0))
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(draft.file_name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(9.0))
                                        .text_color(ui::text_secondary())
                                        .line_clamp(1)
                                        .child(draft.remote_path.clone()),
                                ),
                        )
                        .child(status_pill(
                            draft.state.label().to_string(),
                            state_color,
                            dark,
                        )),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(ui::text_secondary())
                        .line_clamp(1)
                        .child(format!("本地缓存: {}", draft.local_cache_path)),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(state_color)
                        .child(draft.message.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            small_action("打开本地文件", dark)
                                .id("ftp-draft-open-local")
                                .on_click({
                                    let panel = panel.clone();
                                    move |_, window, cx| {
                                        cx.update_entity(&panel, |view, _cx| {
                                            view.reopen_local_draft(&draft_for_open)
                                        });
                                        window.refresh();
                                    }
                                }),
                        )
                        .when(draft.state != RemoteEditState::Synced, |row| {
                            let force = draft_for_upload.state == RemoteEditState::ConflictRisk;
                            row.child(
                                small_action(
                                    if force {
                                        "确认覆盖回传"
                                    } else {
                                        "回传"
                                    },
                                    dark,
                                )
                                .id("ftp-draft-upload")
                                .on_click({
                                    let panel = panel.clone();
                                    let draft_id = draft_for_upload.id.clone();
                                    move |_, window, cx| {
                                        cx.update_entity(&panel, |view, _cx| {
                                            view.upload_draft(&draft_id, force)
                                        });
                                        window.refresh();
                                    }
                                }),
                            )
                        }),
                )
        }))
}

fn transfer_card(
    dark: bool,
    item: &TransferItem,
    session_name: Option<String>,
    panel: Entity<FtpSftpSshView>,
) -> impl IntoElement {
    let progress = item.progress_percent() as f32 / 100.0;
    let fill_width = (280.0 * progress.clamp(0.0, 1.0)).max(if progress > 0.0 { 2.0 } else { 0.0 });
    let status_color = match item.status {
        TransferStatus::Queued => ui::text_secondary(),
        TransferStatus::Running => theme::semantic().info,
        TransferStatus::Completed => theme::semantic().success,
        TransferStatus::Failed => theme::semantic().danger,
        TransferStatus::Cancelled => ui::text_tertiary(),
    };
    let transfer_id = item.id.clone();

    div()
        .rounded(px(8.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(ui::bg_keycap())
        .px(px(10.0))
        .py(px(8.0))
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .size(px(20.0))
                                .rounded(px(4.0))
                                .bg(if dark {
                                    hsla(0.0, 0.0, 1.0, 0.05)
                                } else {
                                    hsla(0.56, 0.80, 0.88, 0.24)
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(10.0))
                                .text_color(status_color)
                                .child(item.direction.arrow()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(1.0))
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .line_clamp(1)
                                        .child(item.name.clone()),
                                )
                                .when(session_name.is_some(), |col| {
                                    col.child(
                                        div()
                                            .text_size(px(9.0))
                                            .text_color(ui::text_secondary())
                                            .line_clamp(1)
                                            .child(session_name.unwrap_or_default()),
                                    )
                                }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(9.0))
                                .text_color(status_color)
                                .child(item.status_line()),
                        )
                        .when(item.is_active(), |row| {
                            row.child(
                                small_action("取消", dark)
                                    .id("ftp-transfer-cancel")
                                    .on_click({
                                        let panel = panel.clone();
                                        move |_, window, cx| {
                                            panel.read(cx).service.cancel_transfer(&transfer_id);
                                            window.refresh();
                                        }
                                    }),
                            )
                        }),
                ),
        )
        .child(
            div()
                .h(px(4.0))
                .rounded(px(999.0))
                .bg(if dark {
                    hsla(0.0, 0.0, 1.0, 0.06)
                } else {
                    hsla(0.0, 0.0, 0.5, 0.10)
                })
                .child(
                    div()
                        .h(px(4.0))
                        .w(px(fill_width))
                        .rounded(px(999.0))
                        .bg(status_color),
                ),
        )
        .child(
            div()
                .text_size(px(9.0))
                .text_color(ui::text_secondary())
                .line_clamp(1)
                .child(format!("{} · {}", item.local_path, item.remote_path)),
        )
}

fn profile_editor_overlay(
    dark: bool,
    editor: Option<ProfileEditorSnapshot>,
    _selected: Option<RemoteProfile>,
    panel: Entity<FtpSftpSshView>,
) -> gpui::AnyElement {
    let Some(editor) = editor else {
        return overlay_shell(
            dark,
            "ftp-editor-overlay",
            panel.clone(),
            empty_state_card(dark, "配置编辑器初始化中", "请稍候").into_any_element(),
        )
        .into_any_element();
    };
    let inputs = editor.inputs.clone();
    let protocol = editor.protocol;

    overlay_shell(
        dark,
        "ftp-editor-overlay",
        panel.clone(),
        div()
            .w(px(660.0))
            .max_h(px(620.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(ui::border_light())
            .bg(theme::semantic().bg_surface)
            .shadow_lg()
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(14.0))
                    .py(px(10.0))
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .child(
                        div()
                            .text_size(px(20.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(match editor.mode {
                                ProfileEditorMode::Existing(_) => "编辑连接",
                                ProfileEditorMode::New => "新建连接",
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(ui::text_secondary())
                            .child("为 SFTP / FTP / FTPS 服务器配置访问凭据。"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .id("ftp-scroll")
                    .overflow_y_scroll()
                    .px(px(14.0))
                    .pb(px(10.0))
                    .flex()
                    .flex_col()
                    .gap(px(7.0))
                    .child(editor_notice(editor.notice, dark))
                    .child(profile_form_section(
                        "通用",
                        dark,
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(7.0))
                            .child(profile_inline_field("名称", inputs.name.clone(), dark))
                            .child(profile_label_value_row(
                                "协议",
                                div().flex().gap(px(6.0)).children(
                                    [
                                        RemoteProtocol::Sftp,
                                        RemoteProtocol::Ftp,
                                        RemoteProtocol::Ftps,
                                        RemoteProtocol::Ssh,
                                    ]
                                    .into_iter()
                                    .map(|protocol_item| {
                                        segmented_chip(
                                            protocol_item.label(),
                                            protocol_item == protocol,
                                            dark,
                                        )
                                        .id(("ftp-editor-protocol", protocol_index(protocol_item)))
                                        .on_click({
                                            let panel = panel.clone();
                                            move |_, window, cx| {
                                                cx.update_entity(&panel, |view, _cx| {
                                                    view.set_editor_protocol(protocol_item, _cx)
                                                });
                                                window.refresh();
                                            }
                                        })
                                    }),
                                ),
                                dark,
                            )),
                    ))
                    .child(profile_form_section(
                        "服务器",
                        dark,
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(7.0))
                            .child(profile_inline_field("主机", inputs.host.clone(), dark))
                            .child(profile_inline_field("端口", inputs.port.clone(), dark))
                            .child(profile_inline_field(
                                "用户名",
                                inputs.username.clone(),
                                dark,
                            )),
                    ))
                    .child(profile_form_section(
                        "身份认证",
                        dark,
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(7.0))
                            .child(profile_label_value_row(
                                "方式",
                                div().flex().gap(px(6.0)).children(
                                    [
                                        AuthMethod::Password,
                                        AuthMethod::PrivateKey,
                                        AuthMethod::Agent,
                                    ]
                                    .into_iter()
                                    .map(|auth_method| {
                                        segmented_chip(
                                            auth_method.label(),
                                            auth_method == editor.auth_method,
                                            dark,
                                        )
                                        .id(("ftp-editor-auth", auth_method_index(auth_method)))
                                        .on_click({
                                            let panel = panel.clone();
                                            move |_, window, cx| {
                                                cx.update_entity(&panel, |view, _cx| {
                                                    view.set_editor_auth_method(auth_method)
                                                });
                                                window.refresh();
                                            }
                                        })
                                    }),
                                ),
                                dark,
                            ))
                            .when(editor.auth_method == AuthMethod::Password, |col| {
                                col.child(profile_inline_field(
                                    "密码",
                                    inputs.password.clone(),
                                    dark,
                                ))
                            })
                            .when(editor.auth_method == AuthMethod::PrivateKey, |col| {
                                col.child(profile_inline_field(
                                    "私钥路径",
                                    inputs.private_key_path.clone(),
                                    dark,
                                ))
                                .child(profile_inline_field(
                                    "私钥口令",
                                    inputs.private_key_passphrase.clone(),
                                    dark,
                                ))
                            }),
                    ))
                    .child(
                        div()
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(ui::border_light())
                            .bg(theme::semantic().bg_surface)
                            .px(px(10.0))
                            .py(px(8.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(editor_section_title("高级选项", dark))
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(ui::text_tertiary())
                                            .child("默认目录 · 编码 · 超时 · 跳板机"),
                                    ),
                            )
                            .child(
                                small_action(
                                    if editor.show_advanced {
                                        "收起"
                                    } else {
                                        "展开"
                                    },
                                    dark,
                                )
                                .id("ftp-editor-toggle-advanced")
                                .on_click({
                                    let panel = panel.clone();
                                    move |_, window, cx| {
                                        cx.update_entity(&panel, |view, _cx| {
                                            view.toggle_editor_show_advanced()
                                        });
                                        window.refresh();
                                    }
                                }),
                            ),
                    )
                    .when(editor.show_advanced, |col| {
                        col.child(profile_form_section(
                            "高级配置",
                            dark,
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(7.0))
                                .child(profile_inline_field(
                                    "连接超时(秒)",
                                    inputs.connect_timeout_secs.clone(),
                                    dark,
                                ))
                                .child(profile_inline_field("编码", inputs.encoding.clone(), dark))
                                .when(protocol != RemoteProtocol::Ssh, |advanced| {
                                    advanced.child(profile_inline_field(
                                        "默认远程目录",
                                        inputs.remote_dir.clone(),
                                        dark,
                                    ))
                                })
                                .child(profile_inline_field(
                                    "本地默认目录",
                                    inputs.local_dir.clone(),
                                    dark,
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(6.0))
                                        .child(
                                            segmented_chip(
                                                "FTP 被动模式",
                                                editor.passive_mode,
                                                dark,
                                            )
                                            .id("ftp-editor-passive")
                                            .on_click(
                                                {
                                                    let panel = panel.clone();
                                                    move |_, window, cx| {
                                                        cx.update_entity(&panel, |view, _cx| {
                                                            view.toggle_editor_passive_mode()
                                                        });
                                                        window.refresh();
                                                    }
                                                },
                                            ),
                                        )
                                        .child(
                                            segmented_chip("使用跳板机", editor.jump_enabled, dark)
                                                .id("ftp-editor-jump")
                                                .on_click({
                                                    let panel = panel.clone();
                                                    move |_, window, cx| {
                                                        cx.update_entity(&panel, |view, _cx| {
                                                            view.toggle_editor_jump_enabled()
                                                        });
                                                        window.refresh();
                                                    }
                                                }),
                                        )
                                        .child(
                                            segmented_chip("固定到顶部", editor.pinned, dark)
                                                .id("ftp-editor-pinned")
                                                .on_click({
                                                    let panel = panel.clone();
                                                    move |_, window, cx| {
                                                        cx.update_entity(&panel, |view, _cx| {
                                                            view.toggle_editor_pinned()
                                                        });
                                                        window.refresh();
                                                    }
                                                }),
                                        ),
                                )
                                .when(editor.jump_enabled, |jump| {
                                    jump.child(profile_inline_field(
                                        "跳板机主机",
                                        inputs.jump_host.clone(),
                                        dark,
                                    ))
                                    .child(profile_inline_field(
                                        "跳板机端口",
                                        inputs.jump_port.clone(),
                                        dark,
                                    ))
                                    .child(profile_inline_field(
                                        "跳板机用户",
                                        inputs.jump_username.clone(),
                                        dark,
                                    ))
                                    .child(profile_inline_field(
                                        "跳板机密码",
                                        inputs.jump_password.clone(),
                                        dark,
                                    ))
                                    .child(profile_inline_field(
                                        "跳板机私钥路径",
                                        inputs.jump_private_key_path.clone(),
                                        dark,
                                    ))
                                    .child(
                                        profile_inline_field(
                                            "跳板机私钥口令",
                                            inputs.jump_private_key_passphrase.clone(),
                                            dark,
                                        ),
                                    )
                                })
                                .child(profile_inline_field("备注", inputs.notes.clone(), dark)),
                        ))
                    })
                    .child(protocol_hint_card(
                        dark,
                        protocol,
                        editor.passive_mode,
                        editor.jump_enabled,
                        editor.pinned,
                    )),
            )
            .child(
                div()
                    .px(px(14.0))
                    .py(px(8.0))
                    .border_t_1()
                    .border_color(ui::border_light())
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(ui::text_tertiary())
                            .child("敏感信息以明文方式保存到本地数据库。"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.0))
                            .child(
                                frost_button("取消", None, "ghost", dark, false)
                                    .id("ftp-close-editor")
                                    .on_click({
                                        let panel = panel.clone();
                                        move |_, window, cx| {
                                            cx.update_entity(&panel, |view, _cx| {
                                                view.close_editor()
                                            });
                                            window.refresh();
                                        }
                                    }),
                            )
                            .child(
                                frost_button("保存", None, "primary", dark, false)
                                    .id("ftp-save-editor")
                                    .on_click({
                                        let panel = panel.clone();
                                        move |_, window, cx| {
                                            cx.update_entity(&panel, |view, _cx| {
                                                view.save_editor(_cx)
                                            });
                                            window.refresh();
                                        }
                                    }),
                            ),
                    ),
            ),
    )
    .into_any_element()
}

fn profile_form_section(title: &'static str, _dark: bool, body: gpui::Div) -> gpui::Div {
    div()
        .rounded(px(6.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(theme::semantic().bg_surface)
        .px(px(10.0))
        .py(px(8.0))
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::semantic().text_primary)
                .child(title),
        )
        .child(body)
}

fn profile_label_value_row(label: &'static str, content: gpui::Div, _dark: bool) -> gpui::Div {
    div()
        .border_b_1()
        .border_color(ui::border_light())
        .h(px(36.0))
        .px(px(2.0))
        .flex()
        .items_center()
        .gap(px(7.0))
        .child(
            div()
                .w(px(60.0))
                .text_size(px(10.0))
                .text_color(theme::semantic().text_primary)
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .h(px(30.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(ui::border_light())
                .bg(theme::semantic().bg_elevated)
                .px(px(8.0))
                .flex()
                .items_center()
                .child(content),
        )
}

fn profile_inline_field(label: &'static str, input: Entity<TextInput>, dark: bool) -> gpui::Div {
    profile_label_value_row(label, div().w_full().child(input), dark)
}

fn new_folder_overlay(
    dark: bool,
    input: Option<Entity<TextInput>>,
    panel: Entity<FtpSftpSshView>,
) -> impl IntoElement {
    overlay_shell(
        dark,
        "ftp-folder-overlay",
        panel.clone(),
        div()
            .w(px(380.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(ui::border_light())
            .bg(theme::semantic().bg_elevated)
            .shadow_lg()
            .p(px(14.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("新建远程目录"),
            )
            .child(
                div()
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(ui::border_light())
                    .bg(ui::bg_keycap())
                    .child(input.unwrap_or_else(|| panic!("new folder input should exist"))),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(6.0))
                    .child(
                        frost_button("取消", None, "ghost", dark, false)
                            .id("ftp-cancel-folder")
                            .on_click({
                                let panel = panel.clone();
                                move |_, window, cx| {
                                    cx.update_entity(&panel, |view, _cx| view.close_folder_sheet());
                                    window.refresh();
                                }
                            }),
                    )
                    .child(
                        frost_button("创建", Some("+"), "primary", dark, false)
                            .id("ftp-confirm-folder")
                            .on_click({
                                let panel = panel.clone();
                                move |_, window, cx| {
                                    cx.update_entity(&panel, |view, _cx| {
                                        view.create_new_folder(_cx)
                                    });
                                    window.refresh();
                                }
                            }),
                    ),
            ),
    )
}

fn profile_menu_overlay(
    dark: bool,
    menu: Option<ProfileMenuState>,
    summaries_by_id: &HashMap<i64, SessionSummary>,
    panel: Entity<FtpSftpSshView>,
) -> gpui::AnyElement {
    let Some(menu) = menu else {
        return div().into_any_element();
    };
    let summary = summaries_by_id.get(&menu.profile.id).cloned();
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
                .id("ftp-profile-menu-backdrop")
                .on_click({
                    let panel = panel.clone();
                    move |_, window, cx| {
                        cx.update_entity(&panel, |view, _cx| view.close_menus());
                        window.refresh();
                    }
                }),
        )
        .child(
            div()
                .absolute()
                .top(menu.position.y)
                .left(menu.position.x)
                .w(px(196.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(ui::border_light())
                .bg(theme::semantic().bg_elevated)
                .shadow_lg()
                .p(px(4.0))
                .flex()
                .flex_col()
                .gap(px(3.0))
                .child(
                    menu_item("连接 / 切换", dark)
                        .id("ftp-menu-connect")
                        .on_click({
                            let panel = panel.clone();
                            let profile_id = menu.profile.id;
                            move |_, window, cx| {
                                cx.update_entity(&panel, |view, _cx| {
                                    view.connect_profile(profile_id)
                                });
                                window.refresh();
                            }
                        }),
                )
                .when(summary.is_some(), |menu_el| {
                    let panel = panel.clone();
                    let profile_id = menu.profile.id;
                    menu_el.child(
                        menu_item("断开该 session", dark)
                            .id("ftp-menu-disconnect")
                            .on_click(move |_, window, cx| {
                                cx.update_entity(&panel, |view, _cx| {
                                    view.disconnect_profile(profile_id)
                                });
                                window.refresh();
                            }),
                    )
                })
                .child(menu_item("编辑配置", dark).id("ftp-menu-edit").on_click({
                    let panel = panel.clone();
                    let profile_id = menu.profile.id;
                    move |_, window, cx| {
                        cx.update_entity(&panel, |view, _cx| {
                            view.open_profile_editor(profile_id, _cx)
                        });
                        window.refresh();
                    }
                }))
                .child(
                    menu_item("复制配置", dark)
                        .id("ftp-menu-duplicate")
                        .on_click({
                            let panel = panel.clone();
                            let profile = menu.profile.clone();
                            move |_, window, cx| {
                                cx.update_entity(&panel, |view, _cx| {
                                    view.duplicate_profile(&profile, _cx)
                                });
                                window.refresh();
                            }
                        }),
                )
                .child(menu_item("删除配置", dark).id("ftp-menu-delete").on_click({
                    let panel = panel.clone();
                    let profile_id = menu.profile.id;
                    move |_, window, cx| {
                        let _ = panel.read(cx).service.delete_profile(profile_id);
                        cx.update_entity(&panel, |view, _cx| view.close_menus());
                        window.refresh();
                    }
                })),
        )
        .into_any_element()
}

fn file_menu_overlay(
    dark: bool,
    menu: Option<FileMenuState>,
    panel: Entity<FtpSftpSshView>,
) -> gpui::AnyElement {
    let Some(menu) = menu else {
        return div().into_any_element();
    };
    let can_open_text = !menu.item.is_dir && looks_like_text_file_name(&menu.item.name);
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
                .id("ftp-file-menu-backdrop")
                .on_click({
                    let panel = panel.clone();
                    move |_, window, cx| {
                        cx.update_entity(&panel, |view, _cx| view.close_menus());
                        window.refresh();
                    }
                }),
        )
        .child(
            div()
                .absolute()
                .top(menu.position.y)
                .left(menu.position.x)
                .w(px(196.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(ui::border_light())
                .bg(theme::semantic().bg_elevated)
                .shadow_lg()
                .p(px(4.0))
                .flex()
                .flex_col()
                .gap(px(3.0))
                .when(menu.item.is_dir, |menu_el| {
                    let panel = panel.clone();
                    let item = menu.item.clone();
                    menu_el.child(
                        menu_item("进入目录", dark)
                            .id("ftp-file-menu-enter")
                            .on_click(move |_, window, cx| {
                                cx.update_entity(&panel, |view, _cx| view.navigate_dir(&item.path));
                                window.refresh();
                            }),
                    )
                })
                .when(!menu.item.is_dir, |menu_el| {
                    let panel = panel.clone();
                    let item = menu.item.clone();
                    menu_el.child(
                        menu_item("下载到本地", dark)
                            .id("ftp-file-menu-download")
                            .on_click(move |_, window, cx| {
                                cx.update_entity(&panel, |view, _cx| view.download_file(&item));
                                window.refresh();
                            }),
                    )
                })
                .when(can_open_text, |menu_el| {
                    let panel = panel.clone();
                    let item = menu.item.clone();
                    menu_el.child(
                        menu_item("打开文本文件", dark)
                            .id("ftp-file-menu-text")
                            .on_click(move |_, window, cx| {
                                cx.update_entity(&panel, |view, _cx| view.open_text_file(&item));
                                window.refresh();
                            }),
                    )
                })
                .child(
                    menu_item("删除", dark)
                        .id("ftp-file-menu-delete")
                        .on_click({
                            let panel = panel.clone();
                            let item = menu.item.clone();
                            move |_, window, cx| {
                                cx.update_entity(&panel, |view, _cx| {
                                    view.delete_remote_item(&item)
                                });
                                window.refresh();
                            }
                        }),
                ),
        )
        .into_any_element()
}

fn overlay_shell(
    dark: bool,
    backdrop_id: &'static str,
    panel: Entity<FtpSftpSshView>,
    content: impl IntoElement,
) -> impl IntoElement {
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
                .bg(hsla(0.0, 0.0, 0.0, if dark { 0.42 } else { 0.24 }))
                .id(backdrop_id)
                .on_click(move |_, window, cx| {
                    cx.update_entity(&panel, |view, _cx| {
                        view.close_menus();
                        view.close_editor();
                        view.close_folder_sheet();
                    });
                    window.refresh();
                }),
        )
        .child(
            div()
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .flex()
                .items_center()
                .justify_center()
                .child(content),
        )
}

fn search_input_shell(input: Option<Entity<TextInput>>, _dark: bool) -> impl IntoElement {
    div()
        .rounded(px(6.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(theme::semantic().bg_surface)
        .child(input.unwrap_or_else(|| panic!("search input should be initialized")))
}

fn protocol_hint_card(
    _dark: bool,
    protocol: RemoteProtocol,
    passive_mode: bool,
    jump_enabled: bool,
    pinned: bool,
) -> impl IntoElement {
    let hint = match protocol {
        RemoteProtocol::Ftp => format!(
            "FTP 会显示文件区和命令日志。当前{}，{}。",
            if passive_mode {
                "使用被动模式"
            } else {
                "使用主动模式"
            },
            if pinned { "已固定" } else { "未固定" }
        ),
        RemoteProtocol::Ftps => String::from("FTPS 保留兼容入口，会按当前后端真实支持能力校验。"),
        RemoteProtocol::Sftp => format!(
            "SFTP 连接会显示远程文件区和 SSH 终端。{}。",
            if jump_enabled {
                "已启用跳板机字段"
            } else {
                "未启用跳板机"
            }
        ),
        RemoteProtocol::Ssh => String::from("SSH 连接只提供终端，不伪装成可浏览文件的页面。"),
    };

    div()
        .rounded(px(8.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(ui::bg_keycap())
        .px(px(10.0))
        .py(px(8.0))
        .text_size(px(9.0))
        .line_height(px(14.0))
        .text_color(ui::text_secondary())
        .child(hint)
}

fn editor_notice(notice: String, _dark: bool) -> impl IntoElement {
    div()
        .text_size(px(10.0))
        .line_height(px(15.0))
        .text_color(ui::text_secondary())
        .child(notice)
}

fn editor_section_title(label: &'static str, _dark: bool) -> impl IntoElement {
    div()
        .text_size(px(9.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ui::text_tertiary())
        .child(label)
}

fn profile_field(label: &'static str, input: Entity<TextInput>, _dark: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .child(
            div()
                .text_size(px(9.0))
                .text_color(ui::text_secondary())
                .child(label),
        )
        .child(
            div()
                .rounded(px(6.0))
                .border_1()
                .border_color(ui::border_light())
                .bg(ui::bg_keycap())
                .child(input),
        )
}

fn segmented_chip(label: &'static str, active: bool, dark: bool) -> gpui::Div {
    div()
        .h(px(22.0))
        .px(px(7.0))
        .rounded(px(6.0))
        .bg(if active {
            if dark {
                hsla(0.54, 0.78, 0.56, 0.18)
            } else {
                hsla(0.54, 0.70, 0.78, 0.14)
            }
        } else {
            ui::bg_keycap()
        })
        .border_1()
        .border_color(if active {
            theme::rgba_with_alpha(
                theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Cyan),
                0.34,
            )
        } else {
            ui::border_light()
        })
        .hover(move |style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(9.0))
        .text_color(if active {
            theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Cyan)
        } else {
            ui::text_secondary()
        })
        .child(label)
}

fn status_pill(label: String, color: gpui::Rgba, _dark: bool) -> impl IntoElement {
    div()
        .h(px(20.0))
        .px(px(6.0))
        .rounded(px(999.0))
        .bg(ui::bg_keycap())
        .border_1()
        .border_color(ui::border_light())
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(div().size(px(5.0)).rounded(px(999.0)).bg(color))
        .child(
            div()
                .text_size(px(8.0))
                .text_color(theme::semantic().text_primary)
                .child(label),
        )
}

fn meta_badge(label: String, color: gpui::Rgba, dark: bool) -> impl IntoElement {
    div()
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(
            color,
            if dark { 0.18 } else { 0.10 },
        ))
        .px(px(6.0))
        .py(px(3.0))
        .text_size(px(8.0))
        .text_color(if dark { color } else { color })
        .child(label)
}

fn small_action(label: &'static str, dark: bool) -> gpui::Div {
    div()
        .h(px(20.0))
        .px(px(5.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(ui::bg_keycap())
        .hover(move |style| style.bg(row_hover_color(dark)).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(9.0))
        .text_color(theme::semantic().text_primary)
        .child(label)
}

fn small_icon_action(icon: IconName, dark: bool) -> gpui::Div {
    div()
        .size(px(22.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(ui::bg_keycap())
        .hover(move |style| style.bg(row_hover_color(dark)).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme::semantic().text_primary)
        .child(Icon::new(icon).with_size(px(12.0)))
}

fn small_icon_text_action(icon: IconName, label: &'static str, dark: bool) -> gpui::Div {
    div()
        .h(px(22.0))
        .px(px(5.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(ui::bg_keycap())
        .hover(move |style| style.bg(row_hover_color(dark)).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .gap(px(3.0))
        .text_size(px(9.0))
        .text_color(theme::semantic().text_primary)
        .child(Icon::new(icon).with_size(px(11.0)))
        .child(label)
}

fn menu_item(label: &'static str, dark: bool) -> gpui::Div {
    div()
        .h(px(26.0))
        .rounded(px(6.0))
        .px(px(8.0))
        .hover(move |style| style.bg(row_hover_color(dark)).cursor_pointer())
        .flex()
        .items_center()
        .text_size(px(9.0))
        .text_color(theme::semantic().text_primary)
        .child(label)
}

fn transfer_count_chip(
    label: &'static str,
    count: usize,
    color: gpui::Rgba,
    dark: bool,
    emphasize: bool,
) -> impl IntoElement {
    div()
        .h(px(16.0))
        .px(px(5.0))
        .rounded(px(999.0))
        .bg(if emphasize {
            theme::rgba_with_alpha(color, if dark { 0.20 } else { 0.12 })
        } else {
            ui::bg_keycap()
        })
        .text_size(px(8.0))
        .text_color(if emphasize {
            color
        } else {
            ui::text_secondary()
        })
        .flex()
        .items_center()
        .justify_center()
        .child(format!("{label} {count}"))
}

fn empty_state_card(_dark: bool, title: &'static str, body: &str) -> impl IntoElement {
    div()
        .rounded(px(8.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(ui::bg_keycap())
        .p(px(12.0))
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .text_size(px(8.0))
                .line_height(px(14.0))
                .text_color(ui::text_secondary())
                .child(body.to_string()),
        )
}

fn status_bar(dark: bool, accent: gpui::Rgba, message: String) -> impl IntoElement {
    div()
        .h(px(26.0))
        .px(px(10.0))
        .border_t_1()
        .border_color(ui::border_light())
        .flex()
        .items_center()
        .child(ui::status_bar(
            message,
            if dark {
                accent
            } else {
                theme::semantic().text_regular
            },
        ))
}

fn protocol_status_text(protocol: RemoteProtocol) -> String {
    match protocol {
        RemoteProtocol::Sftp => String::from("已连接 · SFTP 文件与终端"),
        RemoteProtocol::Ftp => String::from("已连接 · FTP 文件与命令日志"),
        RemoteProtocol::Ftps => String::from("已连接 · FTPS 文件与命令日志"),
        RemoteProtocol::Ssh => String::from("已连接 · SSH 终端"),
    }
}

fn profile_input(
    cx: &mut App,
    placeholder: &str,
    value: &str,
    monospace: bool,
    height: f32,
) -> Entity<TextInput> {
    let placeholder = placeholder.to_string();
    let value = value.to_string();
    cx.new(move |cx| {
        let mut input = TextInput::new(cx, placeholder.clone(), value.clone());
        input.set_chrome(false, cx);
        input.set_monospace(monospace, cx);
        if height > 42.0 {
            input.set_multiline(true, cx);
        }
        input.set_style(
            TextInputStyle {
                height,
                font_size: 11.0,
                padding: 6.0,
            },
            cx,
        );
        input
    })
}

fn set_input_text(input: &Entity<TextInput>, text: String, cx: &mut impl AppContext) {
    input.update(cx, |input, input_cx| input.set_text(text, input_cx));
}

fn parse_u16_or_default(value: &str, fallback: u16) -> u16 {
    value
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn protocol_index(protocol: RemoteProtocol) -> usize {
    match protocol {
        RemoteProtocol::Sftp => 0,
        RemoteProtocol::Ftp => 1,
        RemoteProtocol::Ftps => 2,
        RemoteProtocol::Ssh => 3,
    }
}

fn auth_method_index(auth_method: AuthMethod) -> usize {
    match auth_method {
        AuthMethod::Password => 0,
        AuthMethod::PrivateKey => 1,
        AuthMethod::Agent => 2,
    }
}

fn frost_button(
    label: &'static str,
    icon: Option<&'static str>,
    variant: &'static str,
    dark: bool,
    danger: bool,
) -> gpui::Div {
    let icon = icon.map(SharedString::from);
    ui::ui_button(label, variant, dark, icon, danger).hover(move |style| style.cursor_pointer())
}

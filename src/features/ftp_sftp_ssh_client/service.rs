use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
};

use anyhow::{Context, Result, anyhow, ensure};

use crate::{
    core::storage::AppPaths,
    features::ftp_sftp_ssh_client::{
        backend::{
            RemoteTerminal, create_backend, make_remote_version_hint, now_unix_secs,
            spawn_file_monitor,
        },
        manifest::PLUGIN_ID,
        model::{
            ConnectionStatus, ProtocolLogEntry, RemoteEditDraft, RemoteEditState, RemoteFileItem,
            RemoteProfile, RemoteProfileDraft, RemoteProtocol, RightPanelMode, SessionSummary,
            SessionTransferItem, TerminalSnapshot, TerminalStatus, TransferItem, TransferStatus,
            parent_remote_path,
        },
        pool::RemoteConnectionPool,
        store::RemoteProfileStore,
        transfer::TransferService,
    },
};

#[derive(Clone)]
struct TerminalSessionState {
    terminal: Arc<dyn RemoteTerminal>,
    snapshot: TerminalSnapshot,
}

struct SessionRuntime {
    profile_id: i64,
    profile_name: String,
    protocol: RemoteProtocol,
    status: ConnectionStatus,
    message: String,
    current_remote_path: String,
    remote_items: Vec<RemoteFileItem>,
    ftp_log: Vec<ProtocolLogEntry>,
    terminal: Option<TerminalSessionState>,
    transfer: Arc<TransferService>,
    drafts: HashMap<String, RemoteEditDraft>,
    draft_watchers: HashMap<String, mpsc::Sender<()>>,
}

impl SessionRuntime {
    fn summary(&self) -> SessionSummary {
        let active_transfer_count = self
            .transfer
            .items()
            .into_iter()
            .filter(|item| item.is_active())
            .count();
        let dirty_edit_count = self
            .drafts
            .values()
            .filter(|draft| draft.is_dirty())
            .count();
        SessionSummary {
            profile_id: self.profile_id,
            name: self.profile_name.clone(),
            protocol: self.protocol,
            status: self.status,
            remote_path: self.current_remote_path.clone(),
            right_panel_mode: self.protocol.right_panel_mode(),
            active_transfer_count,
            dirty_edit_count,
            ftp_log_count: self.ftp_log.len(),
            has_session: true,
            last_message: self.message.clone(),
        }
    }
}

pub struct FtpSftpSshService {
    db_path: PathBuf,
    cache_dir: PathBuf,
    sessions: Mutex<HashMap<i64, SessionRuntime>>,
    active_profile_id: Mutex<Option<i64>>,
    revision: Arc<AtomicU64>,
    pool: Arc<RemoteConnectionPool>,
}

impl FtpSftpSshService {
    pub fn new(paths: AppPaths) -> Result<Self> {
        let db_path = paths.feature_state(PLUGIN_ID, "profiles.db");
        let cache_dir = paths.feature_state(PLUGIN_ID, "edited-cache");
        std::fs::create_dir_all(&cache_dir)
            .with_context(|| format!("无法创建缓存目录 {}", cache_dir.display()))?;
        let service = Self {
            db_path,
            cache_dir,
            sessions: Mutex::new(HashMap::new()),
            active_profile_id: Mutex::new(None),
            revision: Arc::new(AtomicU64::new(0)),
            pool: Arc::new(RemoteConnectionPool::new()),
        };
        service.open_store()?.seed_defaults()?;
        if let Some(first) = service.list_profiles()?.first() {
            service.set_active_profile(Some(first.id));
        }
        Ok(service)
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }

    pub fn list_profiles(&self) -> Result<Vec<RemoteProfile>> {
        self.open_store()?.list_profiles()
    }

    pub fn selected_profile_id(&self) -> Option<i64> {
        self.active_profile_id.lock().map(|id| *id).unwrap_or(None)
    }

    pub fn connected_profile_id(&self) -> Option<i64> {
        let active = self.selected_profile_id()?;
        self.sessions
            .lock()
            .ok()
            .and_then(|sessions| sessions.get(&active).map(|_| active))
    }

    pub fn selected_profile(&self) -> Result<Option<RemoteProfile>> {
        let Some(id) = self.selected_profile_id() else {
            return Ok(None);
        };
        self.open_store()?.get_profile(id)
    }

    pub fn set_active_profile(&self, id: Option<i64>) {
        if let Ok(mut active) = self.active_profile_id.lock() {
            *active = id;
        }
        self.bump();
    }

    pub fn select_profile(&self, id: i64) -> Result<()> {
        self.open_store()?
            .get_profile(id)?
            .with_context(|| format!("连接配置不存在: {id}"))?;
        self.set_active_profile(Some(id));
        Ok(())
    }

    pub fn active_session_summary(&self) -> Option<SessionSummary> {
        let active = self.selected_profile_id()?;
        self.session_summary_for(active)
    }

    pub fn session_summary_for(&self, profile_id: i64) -> Option<SessionSummary> {
        self.sessions
            .lock()
            .ok()?
            .get(&profile_id)
            .map(SessionRuntime::summary)
    }

    pub fn session_summaries(&self) -> Vec<SessionSummary> {
        let sessions = self.sessions.lock().map(|guard| {
            let mut values = guard
                .values()
                .map(SessionRuntime::summary)
                .collect::<Vec<_>>();
            values.sort_by(|a, b| a.name.cmp(&b.name));
            values
        });
        sessions.unwrap_or_default()
    }

    pub fn add_demo_profile(&self) -> Result<RemoteProfile> {
        let count = self.list_profiles()?.len();
        let profile = self
            .open_store()?
            .create_profile(&RemoteProfileDraft::demo(count))?;
        self.set_active_profile(Some(profile.id));
        Ok(profile)
    }

    pub fn create_profile(&self, draft: RemoteProfileDraft) -> Result<RemoteProfile> {
        let profile = self.open_store()?.create_profile(&draft)?;
        self.set_active_profile(Some(profile.id));
        Ok(profile)
    }

    pub fn test_profile_draft(&self, draft: RemoteProfileDraft) -> Result<String> {
        let draft = draft.normalize();
        ensure!(!draft.host.trim().is_empty(), "主机不能为空");
        ensure!(!draft.username.trim().is_empty(), "用户名不能为空");
        let profile = RemoteProfile {
            id: -1,
            name: draft.name.clone(),
            protocol: draft.protocol,
            host: draft.host.clone(),
            port: draft.port,
            username: draft.username.clone(),
            auth_method: draft.auth_method,
            password: draft.password.clone(),
            private_key_path: draft.private_key_path.clone(),
            private_key_passphrase: draft.private_key_passphrase.clone(),
            remote_dir: draft.remote_dir.clone(),
            local_dir: draft.local_dir.clone(),
            encoding: draft.encoding.clone(),
            passive_mode: draft.passive_mode,
            connect_timeout_secs: draft.connect_timeout_secs,
            jump_enabled: draft.jump_enabled,
            jump_host: draft.jump_host.clone(),
            jump_port: draft.jump_port,
            jump_username: draft.jump_username.clone(),
            jump_password: draft.jump_password.clone(),
            jump_private_key_path: draft.jump_private_key_path.clone(),
            jump_private_key_passphrase: draft.jump_private_key_passphrase.clone(),
            pinned: draft.pinned,
            notes: draft.notes.clone(),
            last_used_at: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        let mut backend = create_backend(&profile);
        backend.connect()?;
        let capability = match profile.protocol {
            RemoteProtocol::Sftp => "SFTP 文件与终端",
            RemoteProtocol::Ssh => "SSH 终端",
            RemoteProtocol::Ftp => "FTP 文件与命令日志",
            RemoteProtocol::Ftps => "FTPS 兼容入口",
        };
        backend.close();
        Ok(format!("测试连接成功 · {capability}"))
    }

    pub fn update_profile(&self, id: i64, draft: RemoteProfileDraft) -> Result<RemoteProfile> {
        let profile = self
            .open_store()?
            .update_profile(id, &draft)?
            .with_context(|| format!("连接配置不存在: {id}"))?;
        self.disconnect_profile(id);
        self.set_active_profile(Some(id));
        Ok(profile)
    }

    pub fn delete_profile(&self, id: i64) -> Result<bool> {
        let deleted = self.open_store()?.delete_profile(id)?;
        if deleted {
            self.disconnect_profile(id);
            let next = self.list_profiles()?.first().map(|profile| profile.id);
            self.set_active_profile(next);
        }
        Ok(deleted)
    }

    pub fn toggle_pinned(&self, id: i64) -> Result<Option<bool>> {
        let next = self.open_store()?.toggle_pinned(id)?;
        self.bump();
        Ok(next)
    }

    pub fn connect_selected(self: &Arc<Self>) -> Result<()> {
        let profile_id = self.selected_profile_id().context("请先选择连接配置")?;
        self.connect_profile(profile_id)
    }

    pub fn connect_profile(self: &Arc<Self>, profile_id: i64) -> Result<()> {
        let profile = self
            .open_store()?
            .get_profile(profile_id)?
            .with_context(|| format!("连接配置不存在: {profile_id}"))?;
        ensure!(!profile.host.trim().is_empty(), "主机不能为空");
        ensure!(!profile.username.trim().is_empty(), "用户名不能为空");
        self.open_store()?.update_last_used(profile.id)?;
        self.set_active_profile(Some(profile.id));

        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| anyhow!("session 锁被污染"))?;
            if sessions.contains_key(&profile.id) {
                let connected = self.pool.has(profile.id);
                if connected {
                    if let Some(runtime) = sessions.get_mut(&profile.id) {
                        runtime.message = format!("{} 已连接", profile.name);
                        runtime.status = ConnectionStatus::Connected;
                    }
                    self.bump();
                    return Ok(());
                }
                sessions.remove(&profile.id);
            }

            let transfer = Arc::new(TransferService::new(
                Arc::clone(&self.pool),
                Arc::clone(&self.revision),
            ));
            sessions.insert(
                profile.id,
                SessionRuntime {
                    profile_id: profile.id,
                    profile_name: profile.name.clone(),
                    protocol: profile.protocol,
                    status: ConnectionStatus::Idle,
                    message: format!("正在连接 {}...", profile.endpoint()),
                    current_remote_path: profile.remote_dir.clone(),
                    remote_items: vec![RemoteFileItem::status(
                        "连接中".into(),
                        format!("正在连接 {}", profile.endpoint()),
                    )],
                    ftp_log: Vec::new(),
                    terminal: None,
                    transfer,
                    drafts: HashMap::new(),
                    draft_watchers: HashMap::new(),
                },
            );
        }
        self.bump();

        let service = Arc::clone(self);
        thread::spawn(move || {
            let mut backend = create_backend(&profile);
            let result = backend.connect().and_then(|_| {
                let remote_dir = if profile.protocol.supports_file_browser() {
                    if profile.remote_dir.trim().is_empty() || profile.remote_dir == "/" {
                        backend.home_dir().unwrap_or_else(|_| String::from("/"))
                    } else {
                        profile.remote_dir.clone()
                    }
                } else {
                    profile.remote_dir.clone()
                };
                let remote_items = if profile.protocol.supports_file_browser() {
                    backend.list_dir(&remote_dir).unwrap_or_else(|error| {
                        vec![RemoteFileItem::status(
                            "目录读取失败".into(),
                            error.to_string(),
                        )]
                    })
                } else {
                    vec![RemoteFileItem::status(
                        "当前连接不支持文件浏览".into(),
                        "SSH 连接仅提供终端能力".into(),
                    )]
                };
                Ok((remote_dir, remote_items))
            });

            match result {
                Ok((remote_dir, remote_items)) => {
                    service.pool.insert(profile.id, backend);
                    let should_open_terminal = profile.protocol.supports_terminal();
                    if let Ok(mut sessions) = service.sessions.lock() {
                        if let Some(runtime) = sessions.get_mut(&profile.id) {
                            runtime.status = ConnectionStatus::Connected;
                            runtime.message = match profile.protocol {
                                RemoteProtocol::Ftp => format!("已连接 · FTP 文件与命令日志"),
                                RemoteProtocol::Ftps => format!("已连接 · FTPS 文件与命令日志"),
                                RemoteProtocol::Sftp => format!("已连接 · SFTP 文件与终端"),
                                RemoteProtocol::Ssh => format!("已连接 · SSH 终端"),
                            };
                            runtime.current_remote_path = remote_dir;
                            runtime.remote_items = remote_items;
                            runtime.ftp_log = service.protocol_log_for(profile.id);
                        }
                    }
                    if should_open_terminal {
                        let _ = service.open_terminal_for(profile.id);
                    }
                }
                Err(error) => {
                    if let Ok(mut sessions) = service.sessions.lock() {
                        if let Some(runtime) = sessions.get_mut(&profile.id) {
                            runtime.status = ConnectionStatus::Failed;
                            runtime.message = format!("连接失败: {error}");
                            runtime.remote_items =
                                vec![RemoteFileItem::status("连接失败".into(), error.to_string())];
                        }
                    }
                }
            }
            service.bump();
        });

        Ok(())
    }

    pub fn disconnect(&self) {
        if let Some(profile_id) = self.selected_profile_id() {
            self.disconnect_profile(profile_id);
        }
    }

    pub fn disconnect_profile(&self, profile_id: i64) {
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(mut runtime) = sessions.remove(&profile_id) {
                for (_, tx) in runtime.draft_watchers.drain() {
                    let _ = tx.send(());
                }
                if let Some(terminal) = runtime.terminal.take() {
                    terminal.terminal.close();
                }
                runtime.transfer.shutdown();
            }
        }
        if let Some(mut backend) = self.pool.remove(profile_id) {
            backend.close();
        }
        let should_advance = self.selected_profile_id() == Some(profile_id);
        if should_advance {
            let next = self
                .session_summaries()
                .first()
                .map(|summary| summary.profile_id)
                .or_else(|| {
                    self.list_profiles()
                        .ok()
                        .and_then(|profiles| profiles.first().map(|p| p.id))
                });
            self.set_active_profile(next);
        } else {
            self.bump();
        }
    }

    pub fn remote_items(&self) -> Vec<RemoteFileItem> {
        let Some(profile_id) = self.selected_profile_id() else {
            return vec![RemoteFileItem::status(
                "未选择连接".into(),
                "从左侧选择一个连接开始".into(),
            )];
        };
        self.sessions
            .lock()
            .ok()
            .and_then(|sessions| {
                sessions
                    .get(&profile_id)
                    .map(|runtime| runtime.remote_items.clone())
            })
            .unwrap_or_else(|| {
                vec![RemoteFileItem::status(
                    "尚未连接".into(),
                    "双击左侧连接项或点击连接".into(),
                )]
            })
    }

    pub fn current_remote_path(&self) -> String {
        let Some(profile_id) = self.selected_profile_id() else {
            return String::new();
        };
        self.sessions
            .lock()
            .ok()
            .and_then(|sessions| {
                sessions
                    .get(&profile_id)
                    .map(|runtime| runtime.current_remote_path.clone())
            })
            .unwrap_or_default()
    }

    pub fn active_message(&self) -> String {
        let Some(profile_id) = self.selected_profile_id() else {
            return String::from("连接配置已就绪");
        };
        self.sessions
            .lock()
            .ok()
            .and_then(|sessions| {
                sessions
                    .get(&profile_id)
                    .map(|runtime| runtime.message.clone())
            })
            .unwrap_or_else(|| String::from("连接配置已就绪"))
    }

    pub fn message(&self) -> String {
        self.active_message()
    }

    pub fn active_status(&self) -> ConnectionStatus {
        let Some(profile_id) = self.selected_profile_id() else {
            return ConnectionStatus::Idle;
        };
        self.sessions
            .lock()
            .ok()
            .and_then(|sessions| sessions.get(&profile_id).map(|runtime| runtime.status))
            .unwrap_or(ConnectionStatus::Idle)
    }

    pub fn status(&self) -> ConnectionStatus {
        self.active_status()
    }

    pub fn active_right_panel_mode(&self) -> RightPanelMode {
        self.selected_profile()
            .ok()
            .flatten()
            .map(|profile| profile.protocol.right_panel_mode())
            .unwrap_or(RightPanelMode::Empty)
    }

    pub fn protocol_log_for(&self, profile_id: i64) -> Vec<ProtocolLogEntry> {
        self.pool
            .with_backend(profile_id, |backend| backend.protocol_log_snapshot())
            .unwrap_or_default()
    }

    pub fn clear_protocol_log(&self, profile_id: i64) {
        let _ = self
            .pool
            .with_backend(profile_id, |backend| backend.clear_protocol_log());
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(runtime) = sessions.get_mut(&profile_id) {
                runtime.ftp_log.clear();
            }
        }
        self.bump();
    }

    pub fn active_protocol_log(&self) -> Vec<ProtocolLogEntry> {
        let Some(profile_id) = self.selected_profile_id() else {
            return Vec::new();
        };
        let latest = self.protocol_log_for(profile_id);
        let mut changed = false;
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(runtime) = sessions.get_mut(&profile_id) {
                if runtime.ftp_log != latest {
                    runtime.ftp_log = latest.clone();
                    changed = true;
                }
            }
        }
        if changed {
            self.bump();
        }
        latest
    }

    pub fn active_terminal_snapshot(&self) -> TerminalSnapshot {
        let Some(profile_id) = self.selected_profile_id() else {
            return TerminalSnapshot::empty();
        };
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(runtime) = sessions.get_mut(&profile_id) {
                if let Some(terminal) = runtime.terminal.as_mut() {
                    let mut changed = false;
                    let lines = terminal.terminal.try_read();
                    if !lines.is_empty() {
                        terminal.snapshot.lines.extend(lines);
                        if terminal.snapshot.lines.len() > 400 {
                            let drain = terminal.snapshot.lines.len().saturating_sub(400);
                            terminal.snapshot.lines.drain(0..drain);
                        }
                        changed = true;
                    }
                    let next_cwd = terminal.terminal.cwd_hint();
                    if terminal.snapshot.cwd_hint != next_cwd {
                        terminal.snapshot.cwd_hint = next_cwd;
                        changed = true;
                    }
                    if changed {
                        self.bump();
                    }
                    return terminal.snapshot.clone();
                }
            }
        }
        TerminalSnapshot::empty()
    }

    pub fn has_live_terminal(&self) -> bool {
        self.sessions
            .lock()
            .map(|sessions| {
                sessions
                    .values()
                    .any(|runtime| runtime.terminal.as_ref().is_some())
            })
            .unwrap_or(false)
    }

    pub fn open_terminal(&self) -> Result<()> {
        let profile = self.selected_profile()?.context("请先选择连接配置")?;
        self.open_terminal_for(profile.id)
    }

    pub fn open_terminal_for(&self, profile_id: i64) -> Result<()> {
        let profile = self
            .open_store()?
            .get_profile(profile_id)?
            .with_context(|| format!("连接配置不存在: {profile_id}"))?;
        ensure!(profile.protocol.supports_terminal(), "当前连接不支持终端");
        let Some(terminal) = self
            .pool
            .with_backend(profile.id, |backend| backend.open_terminal())
            .transpose()?
        else {
            return Err(anyhow::anyhow!("连接已断开"));
        };
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(runtime) = sessions.get_mut(&profile.id) {
                if let Some(previous) = runtime.terminal.take() {
                    previous.terminal.close();
                }
                runtime.terminal = Some(TerminalSessionState {
                    terminal,
                    snapshot: TerminalSnapshot {
                        status: TerminalStatus::Connected,
                        cwd_hint: runtime.current_remote_path.clone(),
                        lines: vec![format!("已打开 {} 终端", profile.name)],
                    },
                });
            }
        }
        self.bump();
        Ok(())
    }

    pub fn close_terminal(&self) {
        if let Some(profile_id) = self.selected_profile_id() {
            if let Ok(mut sessions) = self.sessions.lock() {
                if let Some(runtime) = sessions.get_mut(&profile_id) {
                    if let Some(terminal) = runtime.terminal.take() {
                        terminal.terminal.close();
                    }
                }
            }
            self.bump();
        }
    }

    pub fn send_terminal_input(&self, text: &str) -> Result<()> {
        let profile_id = self.selected_profile_id().context("未选择连接")?;
        let terminal = self
            .sessions
            .lock()
            .ok()
            .and_then(|sessions| {
                sessions.get(&profile_id).and_then(|runtime| {
                    runtime
                        .terminal
                        .as_ref()
                        .map(|terminal| Arc::clone(&terminal.terminal))
                })
            })
            .context("终端未启动")?;
        terminal.write_input(text)
    }

    pub fn sync_terminal_to_current_dir(&self) -> Result<()> {
        let path = self.current_remote_path();
        ensure!(!path.is_empty(), "当前目录为空");
        self.send_terminal_input(&format!("cd {}\n", escape_shell_path(&path)))
    }

    pub fn navigate_dir(self: &Arc<Self>, path: &str) -> Result<()> {
        let profile_id = self.selected_profile_id().context("未选择连接")?;
        let profile = self.selected_profile()?.context("连接配置不存在")?;
        ensure!(
            profile.protocol.supports_file_browser(),
            "当前连接不支持文件浏览"
        );
        let path = path.to_string();
        let service = Arc::clone(self);
        thread::spawn(move || {
            let items = service
                .pool
                .with_backend(profile_id, |backend| backend.list_dir(&path));
            match items {
                Some(Ok(items)) => {
                    if let Ok(mut sessions) = service.sessions.lock() {
                        if let Some(runtime) = sessions.get_mut(&profile_id) {
                            runtime.current_remote_path = path;
                            runtime.remote_items = items;
                            runtime.message = format!("已切换到 {}", runtime.current_remote_path);
                        }
                    }
                }
                Some(Err(error)) => {
                    if let Ok(mut sessions) = service.sessions.lock() {
                        if let Some(runtime) = sessions.get_mut(&profile_id) {
                            runtime.message = format!("目录读取失败: {error}");
                        }
                    }
                }
                None => {}
            }
            service.bump();
        });
        Ok(())
    }

    pub fn navigate_up(self: &Arc<Self>) -> Result<()> {
        let current = self.current_remote_path();
        if current.is_empty() || current == "/" {
            return Ok(());
        }
        let parent = parent_remote_path(&current);
        self.navigate_dir(&parent)
    }

    pub fn refresh_dir(self: &Arc<Self>) -> Result<()> {
        let current = self.current_remote_path();
        if current.is_empty() {
            return Ok(());
        }
        self.navigate_dir(&current)
    }

    pub fn remote_mkdir(self: &Arc<Self>, path: &str) -> Result<()> {
        let profile_id = self.selected_profile_id().context("未选择连接")?;
        let path = path.to_string();
        let service = Arc::clone(self);
        thread::spawn(move || {
            let result = service
                .pool
                .with_backend(profile_id, |backend| backend.mkdir(&path));
            match result {
                Some(Ok(())) => {
                    if let Ok(mut sessions) = service.sessions.lock() {
                        if let Some(runtime) = sessions.get_mut(&profile_id) {
                            runtime.message = String::from("已创建目录");
                        }
                    }
                    let _ = service.refresh_dir();
                }
                Some(Err(error)) => {
                    if let Ok(mut sessions) = service.sessions.lock() {
                        if let Some(runtime) = sessions.get_mut(&profile_id) {
                            runtime.message = format!("创建目录失败: {error}");
                        }
                    }
                    service.bump();
                }
                None => {}
            }
        });
        Ok(())
    }

    pub fn remote_delete(self: &Arc<Self>, path: &str, is_dir: bool) -> Result<()> {
        let profile_id = self.selected_profile_id().context("未选择连接")?;
        let path = path.to_string();
        let service = Arc::clone(self);
        thread::spawn(move || {
            let result = service.pool.with_backend(profile_id, |backend| {
                if is_dir {
                    backend.delete_dir(&path)
                } else {
                    backend.delete_file(&path)
                }
            });
            match result {
                Some(Ok(())) => {
                    if let Ok(mut sessions) = service.sessions.lock() {
                        if let Some(runtime) = sessions.get_mut(&profile_id) {
                            runtime.message = String::from("已删除");
                        }
                    }
                    let _ = service.refresh_dir();
                }
                Some(Err(error)) => {
                    if let Ok(mut sessions) = service.sessions.lock() {
                        if let Some(runtime) = sessions.get_mut(&profile_id) {
                            runtime.message = format!("删除失败: {error}");
                        }
                    }
                    service.bump();
                }
                None => {}
            }
        });
        Ok(())
    }

    pub fn remote_rename(self: &Arc<Self>, source: &str, target: &str) -> Result<()> {
        let profile_id = self.selected_profile_id().context("未选择连接")?;
        let source = source.to_string();
        let target = target.to_string();
        let service = Arc::clone(self);
        thread::spawn(move || {
            let result = service
                .pool
                .with_backend(profile_id, |backend| backend.rename(&source, &target));
            match result {
                Some(Ok(())) => {
                    if let Ok(mut sessions) = service.sessions.lock() {
                        if let Some(runtime) = sessions.get_mut(&profile_id) {
                            runtime.message = String::from("已重命名");
                        }
                    }
                    let _ = service.refresh_dir();
                }
                Some(Err(error)) => {
                    if let Ok(mut sessions) = service.sessions.lock() {
                        if let Some(runtime) = sessions.get_mut(&profile_id) {
                            runtime.message = format!("重命名失败: {error}");
                        }
                    }
                    service.bump();
                }
                None => {}
            }
        });
        Ok(())
    }

    pub fn upload_paths(self: &Arc<Self>, local_paths: Vec<String>) -> Result<Vec<String>> {
        let profile_id = self.selected_profile_id().context("未选择连接")?;
        let remote_dir = self.current_remote_path();
        ensure!(!remote_dir.is_empty(), "当前目录为空");
        let mut ids = Vec::new();
        for raw in local_paths {
            let path = PathBuf::from(raw);
            if !path.exists() {
                continue;
            }
            if path.is_file() {
                ids.push(self.start_upload(path.to_string_lossy().into_owned(), &remote_dir)?);
                continue;
            }
            if path.is_dir() {
                let root_name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| String::from("目录"));
                let root_remote = join_remote_path_owned(&remote_dir, &root_name);
                let _ = self.remote_mkdir(&root_remote);
                for child in walk_dir_files(&path)? {
                    let rel = child.strip_prefix(&path).unwrap_or(&child);
                    let remote_target =
                        rel.components()
                            .fold(root_remote.clone(), |base, component| {
                                join_remote_path_owned(
                                    &base,
                                    &component.as_os_str().to_string_lossy(),
                                )
                            });
                    if child.is_dir() {
                        let _ = self.remote_mkdir(&remote_target);
                    } else if child.is_file() {
                        ids.push(self.start_upload(
                            child.to_string_lossy().into_owned(),
                            &parent_remote_path_owned(&remote_target),
                        )?);
                    }
                }
            }
        }
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(runtime) = sessions.get_mut(&profile_id) {
                runtime.message = format!("已加入 {} 个传输任务", ids.len());
            }
        }
        self.bump();
        Ok(ids)
    }

    pub fn start_upload(self: &Arc<Self>, local_path: String, remote_dir: &str) -> Result<String> {
        let profile_id = self.selected_profile_id().context("未选择连接")?;
        let profile = self.selected_profile()?.context("连接配置不存在")?;
        let transfer = self
            .sessions
            .lock()
            .ok()
            .and_then(|sessions| {
                sessions
                    .get(&profile_id)
                    .map(|runtime| Arc::clone(&runtime.transfer))
            })
            .context("连接未建立")?;
        let file_name = std::path::Path::new(&local_path)
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        let remote_path = join_remote_path_owned(remote_dir, &file_name);
        let id = transfer.start_upload(profile.id, local_path, remote_path);
        self.bump();
        Ok(id)
    }

    pub fn start_download(
        self: &Arc<Self>,
        remote_path: String,
        local_dir: &str,
        size: i64,
    ) -> Result<String> {
        let profile_id = self.selected_profile_id().context("未选择连接")?;
        let profile = self.selected_profile()?.context("连接配置不存在")?;
        let transfer = self
            .sessions
            .lock()
            .ok()
            .and_then(|sessions| {
                sessions
                    .get(&profile_id)
                    .map(|runtime| Arc::clone(&runtime.transfer))
            })
            .context("连接未建立")?;
        let file_name = std::path::Path::new(&remote_path)
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        let local_path = Path::new(local_dir).join(file_name);
        let id = transfer.start_download(
            profile.id,
            remote_path,
            local_path.to_string_lossy().into_owned(),
            size,
        );
        self.bump();
        Ok(id)
    }

    pub fn transfer_items(&self) -> Vec<TransferItem> {
        let Some(profile_id) = self.selected_profile_id() else {
            return Vec::new();
        };
        self.sessions
            .lock()
            .ok()
            .and_then(|sessions| {
                sessions
                    .get(&profile_id)
                    .map(|runtime| runtime.transfer.items())
            })
            .unwrap_or_default()
    }

    pub fn transfer_item(&self, profile_id: i64, transfer_id: &str) -> Option<TransferItem> {
        self.sessions.lock().ok().and_then(|sessions| {
            sessions
                .get(&profile_id)
                .and_then(|runtime| runtime.transfer.item(transfer_id))
        })
    }

    pub fn all_transfer_items(&self) -> Vec<SessionTransferItem> {
        self.sessions
            .lock()
            .map(|sessions| {
                let mut result = Vec::new();
                for runtime in sessions.values() {
                    for item in runtime.transfer.items() {
                        result.push(SessionTransferItem {
                            profile_id: runtime.profile_id,
                            session_name: runtime.profile_name.clone(),
                            item,
                        });
                    }
                }
                result
            })
            .unwrap_or_default()
    }

    pub fn cancel_transfer(&self, id: &str) {
        let Some(profile_id) = self.selected_profile_id() else {
            return;
        };
        if let Ok(sessions) = self.sessions.lock() {
            if let Some(runtime) = sessions.get(&profile_id) {
                runtime.transfer.cancel(id);
            }
        }
        self.bump();
    }

    pub fn clear_finished_transfers(&self) {
        let Some(profile_id) = self.selected_profile_id() else {
            return;
        };
        if let Ok(sessions) = self.sessions.lock() {
            if let Some(runtime) = sessions.get(&profile_id) {
                runtime.transfer.clear_finished();
            }
        }
        self.bump();
    }

    pub fn remote_edit_drafts(&self) -> Vec<RemoteEditDraft> {
        let Some(profile_id) = self.selected_profile_id() else {
            return Vec::new();
        };
        self.sessions
            .lock()
            .ok()
            .and_then(|sessions| {
                sessions
                    .get(&profile_id)
                    .map(|runtime| runtime.drafts.values().cloned().collect())
            })
            .unwrap_or_default()
    }

    pub fn open_text_file(self: &Arc<Self>, item: &RemoteFileItem) -> Result<()> {
        ensure!(!item.is_dir, "目录不能作为文本文件打开");
        ensure!(
            looks_like_text_file(&item.name),
            "当前文件不支持文本编辑回传"
        );
        let profile_id = self.selected_profile_id().context("未选择连接")?;
        let sanitized_remote = item
            .path
            .trim_matches('/')
            .replace('/', "__")
            .replace(':', "_");
        let cache_path = self
            .cache_dir
            .join(format!("{profile_id}-{sanitized_remote}"));
        let cache_dir = cache_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.cache_dir.clone());
        std::fs::create_dir_all(&cache_dir).context("创建文本缓存目录失败")?;

        let service = Arc::clone(self);
        let remote_path = item.path.clone();
        let file_name = item.name.clone();
        let local_path = cache_path.to_string_lossy().into_owned();
        let size = item.size;
        let version_hint = make_remote_version_hint(item.size, item.modified_at);
        thread::spawn(move || {
            let transfer_id =
                service.start_download(remote_path.clone(), &cache_dir.to_string_lossy(), size);

            let result = match transfer_id {
                Ok(transfer_id) => wait_for_transfer_result(
                    Arc::clone(&service),
                    profile_id,
                    transfer_id,
                    std::time::Duration::from_secs(60),
                ),
                Err(error) => Err(error),
            };

            match result {
                Ok(()) => {
                    let _ = crate::platform::shell::open_path(Path::new(&local_path));
                    let draft_id = format!("{profile_id}:{remote_path}");
                    let monitor = {
                        let service = Arc::clone(&service);
                        let draft_id = draft_id.clone();
                        spawn_file_monitor(local_path.clone(), 800, move |modified_at| {
                            service.mark_draft_modified(profile_id, &draft_id, modified_at);
                        })
                    };
                    if let Ok(mut sessions) = service.sessions.lock() {
                        if let Some(runtime) = sessions.get_mut(&profile_id) {
                            runtime.drafts.insert(
                                draft_id.clone(),
                                RemoteEditDraft {
                                    id: draft_id.clone(),
                                    profile_id,
                                    file_name,
                                    remote_path,
                                    local_cache_path: local_path,
                                    remote_version_hint: version_hint,
                                    last_local_modified_at: now_unix_secs(),
                                    state: RemoteEditState::Synced,
                                    message: String::from("已下载到本地缓存"),
                                },
                            );
                            runtime.draft_watchers.insert(draft_id, monitor);
                            runtime.message = String::from("文本文件已打开，修改后可确认回传");
                        }
                    }
                }
                Err(error) => {
                    if let Ok(mut sessions) = service.sessions.lock() {
                        if let Some(runtime) = sessions.get_mut(&profile_id) {
                            runtime.message = format!("打开文本文件失败: {error}");
                        }
                    }
                }
            }
            service.bump();
        });
        Ok(())
    }

    fn mark_draft_modified(&self, profile_id: i64, draft_id: &str, modified_at: i64) {
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(runtime) = sessions.get_mut(&profile_id) {
                if let Some(draft) = runtime.drafts.get_mut(draft_id) {
                    draft.last_local_modified_at = modified_at;
                    if draft.state == RemoteEditState::Synced {
                        draft.state = RemoteEditState::ModifiedLocal;
                        draft.message = String::from("本地文件已修改，等待确认回传");
                    }
                }
            }
        }
        self.bump();
    }

    pub fn upload_draft(self: &Arc<Self>, draft_id: &str, force: bool) -> Result<()> {
        let profile_id = self.selected_profile_id().context("未选择连接")?;
        let (local_cache_path, remote_path, current_version) = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| anyhow::anyhow!("session 锁被污染"))?;
            let runtime = sessions.get(&profile_id).context("连接未建立")?;
            let draft = runtime.drafts.get(draft_id).context("草稿不存在")?;
            (
                draft.local_cache_path.clone(),
                draft.remote_path.clone(),
                draft.remote_version_hint.clone(),
            )
        };
        let latest = self
            .pool
            .with_backend(profile_id, |backend| {
                backend.remote_file_version(&remote_path)
            })
            .transpose()?
            .context("连接已断开")?;
        if latest != current_version && !force {
            if let Ok(mut sessions) = self.sessions.lock() {
                if let Some(runtime) = sessions.get_mut(&profile_id) {
                    if let Some(draft) = runtime.drafts.get_mut(draft_id) {
                        draft.state = RemoteEditState::ConflictRisk;
                        draft.message = String::from("远程文件可能已变化，请再次确认覆盖");
                    }
                }
            }
            self.bump();
            return Ok(());
        }
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(runtime) = sessions.get_mut(&profile_id) {
                if let Some(draft) = runtime.drafts.get_mut(draft_id) {
                    draft.state = RemoteEditState::UploadingBack;
                    draft.message = String::from("正在回传");
                }
            }
        }
        self.bump();

        let service = Arc::clone(self);
        let draft_id_owned = draft_id.to_string();
        thread::spawn(move || {
            let result = service.pool.with_backend(profile_id, |backend| {
                backend.upload_file(Path::new(&local_cache_path), &remote_path, &|_, _| {})
            });
            if let Ok(mut sessions) = service.sessions.lock() {
                if let Some(runtime) = sessions.get_mut(&profile_id) {
                    if let Some(draft) = runtime.drafts.get_mut(&draft_id_owned) {
                        match result {
                            Some(Ok(())) => {
                                draft.state = RemoteEditState::Synced;
                                draft.remote_version_hint = service
                                    .pool
                                    .with_backend(profile_id, |backend| {
                                        backend.remote_file_version(&remote_path)
                                    })
                                    .transpose()
                                    .ok()
                                    .flatten()
                                    .unwrap_or_else(|| current_version.clone());
                                draft.message = String::from("已回传到远程");
                            }
                            Some(Err(error)) => {
                                draft.state = RemoteEditState::UploadFailed;
                                draft.message = format!("回传失败: {error}");
                            }
                            None => {
                                draft.state = RemoteEditState::UploadFailed;
                                draft.message = String::from("连接已断开");
                            }
                        }
                    }
                }
            }
            service.bump();
        });
        Ok(())
    }

    pub(crate) fn open_store(&self) -> Result<RemoteProfileStore> {
        RemoteProfileStore::open(&self.db_path)
    }

    pub(crate) fn bump(&self) {
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    pub fn shutdown(&self) {
        let profile_ids = self
            .sessions
            .lock()
            .map(|sessions| sessions.keys().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        for profile_id in profile_ids {
            self.disconnect_profile(profile_id);
        }
        self.pool.close_all();
    }
}

fn join_remote_path_owned(base: &str, name: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}

fn parent_remote_path_owned(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) => String::from("/"),
        Some(index) => trimmed[..index].to_string(),
        None => String::from("/"),
    }
}

fn walk_dir_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut all = vec![root.to_path_buf()];
    if root.is_dir() {
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                all.extend(walk_dir_files(&path)?);
            } else {
                all.push(path);
            }
        }
    }
    Ok(all)
}

fn looks_like_text_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let allowed = [
        ".txt", ".md", ".json", ".yml", ".yaml", ".toml", ".ini", ".env", ".xml", ".html", ".css",
        ".js", ".ts", ".rs", ".py", ".sh", ".conf", ".log",
    ];
    allowed.iter().any(|ext| lower.ends_with(ext))
}

fn escape_shell_path(path: &str) -> String {
    path.replace('\'', "'\\''")
}

fn wait_for_transfer_result(
    service: Arc<FtpSftpSshService>,
    profile_id: i64,
    transfer_id: String,
    timeout: std::time::Duration,
) -> Result<()> {
    let started = std::time::Instant::now();
    loop {
        let item = service
            .transfer_item(profile_id, &transfer_id)
            .with_context(|| format!("传输任务不存在: {transfer_id}"))?;
        match item.status {
            TransferStatus::Completed => return Ok(()),
            TransferStatus::Failed => return Err(anyhow!(item.message)),
            TransferStatus::Cancelled => return Err(anyhow!("传输已取消")),
            TransferStatus::Queued | TransferStatus::Running => {}
        }
        if started.elapsed() > timeout {
            return Err(anyhow!("等待传输完成超时"));
        }
        thread::sleep(std::time::Duration::from_millis(120));
    }
}

pub(crate) fn looks_like_text_file_name(name: &str) -> bool {
    looks_like_text_file(name)
}

#[cfg(test)]
mod tests {
    use super::looks_like_text_file_name;
    use crate::features::ftp_sftp_ssh_client::model::{RemoteProtocol, RightPanelMode};

    #[test]
    fn protocol_capabilities_match_workspace_rules() {
        assert!(RemoteProtocol::Sftp.supports_file_browser());
        assert!(RemoteProtocol::Sftp.supports_terminal());
        assert_eq!(
            RemoteProtocol::Sftp.right_panel_mode(),
            RightPanelMode::Terminal
        );

        assert!(RemoteProtocol::Ftp.supports_file_browser());
        assert!(!RemoteProtocol::Ftp.supports_terminal());
        assert_eq!(
            RemoteProtocol::Ftp.right_panel_mode(),
            RightPanelMode::FtpLog
        );

        assert!(!RemoteProtocol::Ssh.supports_file_browser());
        assert!(RemoteProtocol::Ssh.supports_terminal());
        assert_eq!(
            RemoteProtocol::Ssh.right_panel_mode(),
            RightPanelMode::Terminal
        );
    }

    #[test]
    fn text_file_detection_covers_edit_back_flow() {
        assert!(looks_like_text_file_name("nginx.conf"));
        assert!(looks_like_text_file_name("README.MD"));
        assert!(looks_like_text_file_name("app.ts"));
        assert!(!looks_like_text_file_name("archive.zip"));
        assert!(!looks_like_text_file_name("photo.png"));
    }
}

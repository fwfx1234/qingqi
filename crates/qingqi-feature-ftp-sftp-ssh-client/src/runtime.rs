use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
};

use anyhow::{Context, Result, anyhow};
use russh::ChannelMsg;
use tokio::sync::mpsc as tokio_mpsc;

use crate::{
    manifest::PLUGIN_ID,
    model::{Profile, ProfileDraft, RemoteProtocol, SessionId, SessionStatus, SessionSummary},
    protocols::{ConnectionHealth, RemoteEntry, connect_ssh, create_file_client},
    store::ProfileStore,
    terminal::{TerminalEngine, TerminalFrame, TerminalInput},
    transfer::{TransferDirection, TransferQueue, TransferSnapshot},
};
use qingqi_plugin::{
    database::{DatabaseService, feature_database_key},
    storage::AppPaths,
};

#[derive(Clone, Debug)]
pub struct EditableFile {
    pub local_path: PathBuf,
    pub remote_path: String,
}

#[derive(Clone, Debug)]
pub struct SessionSnapshot {
    pub summary: SessionSummary,
    pub connection: ConnectionHealth,
    pub remote_root: String,
    pub remote_entries: Vec<RemoteEntry>,
    pub local_root: String,
    pub editable_files: Vec<EditableFile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteRuntimeEvent {
    ProfilesChanged,
    SessionsChanged,
    SessionChanged(SessionId),
    TransfersChanged,
    TerminalChanged(SessionId),
}

#[derive(Clone, Default)]
struct RuntimeEventBus {
    subscribers: Arc<Mutex<Vec<mpsc::Sender<RemoteRuntimeEvent>>>>,
}

impl RuntimeEventBus {
    fn emit(&self, event: RemoteRuntimeEvent) {
        if let Ok(mut subscribers) = self.subscribers.lock() {
            subscribers.retain(|subscriber| subscriber.send(event.clone()).is_ok());
        }
    }

    fn subscribe(&self) -> mpsc::Receiver<RemoteRuntimeEvent> {
        let (sender, receiver) = mpsc::channel();
        if let Ok(mut subscribers) = self.subscribers.lock() {
            subscribers.push(sender);
        }
        receiver
    }
}

struct SessionRuntime {
    profile: Profile,
    snapshot: SessionSnapshot,
    terminal: Option<LiveTerminal>,
}

enum TerminalCommand {
    Input(Vec<u8>),
    Resize { columns: u32, rows: u32 },
    Shutdown,
}

struct LiveTerminal {
    engine: Arc<Mutex<TerminalEngine>>,
    command_tx: tokio_mpsc::UnboundedSender<TerminalCommand>,
}

impl LiveTerminal {
    fn spawn(profile: Profile, session_id: SessionId, events: RuntimeEventBus) -> Result<Self> {
        let engine = Arc::new(Mutex::new(TerminalEngine::new(profile.name.clone())));
        let command_tx = spawn_terminal_thread(profile, session_id, Arc::clone(&engine), events)?;
        Ok(Self { engine, command_tx })
    }

    fn frame(&self) -> Option<TerminalFrame> {
        self.engine.lock().ok().map(|engine| engine.frame())
    }

    fn revision(&self) -> Option<u64> {
        self.engine.lock().ok().map(|engine| engine.revision())
    }

    #[allow(dead_code)]
    fn send_input(&self, input: TerminalInput) -> Result<()> {
        let bytes = self
            .engine
            .lock()
            .map_err(|_| anyhow!("terminal engine lock poisoned"))?
            .encode_input(input);
        self.command_tx
            .send(TerminalCommand::Input(bytes))
            .map_err(|_| anyhow!("terminal session already closed"))
    }

    #[allow(dead_code)]
    fn resize(&self, columns: u32, rows: u32) -> Result<()> {
        if let Ok(mut engine) = self.engine.lock() {
            let _ = engine.resize(columns as usize, rows as usize);
        }
        self.command_tx
            .send(TerminalCommand::Resize { columns, rows })
            .map_err(|_| anyhow!("terminal session already closed"))
    }
}

impl Drop for LiveTerminal {
    fn drop(&mut self) {
        let _ = self.command_tx.send(TerminalCommand::Shutdown);
    }
}

pub struct RemoteRuntime {
    store: ProfileStore,
    paths: AppPaths,
    sessions: Arc<Mutex<HashMap<SessionId, SessionRuntime>>>,
    transfer_queue: Arc<TransferQueue>,
    events: RuntimeEventBus,
}

impl RemoteRuntime {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> Result<Self> {
        let store = ProfileStore::open(database, &feature_database_key(PLUGIN_ID, "profiles"))?;
        store.seed_defaults()?;
        Ok(Self {
            store,
            paths,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            transfer_queue: Arc::new(TransferQueue::new(3)),
            events: RuntimeEventBus::default(),
        })
    }

    pub fn list_profiles(&self) -> Result<Vec<Profile>> {
        self.store.list_profiles()
    }

    pub fn create_profile(&self, draft: ProfileDraft) -> Result<Profile> {
        let profile = self.store.create_profile(&draft)?;
        self.events.emit(RemoteRuntimeEvent::ProfilesChanged);
        Ok(profile)
    }

    pub fn update_profile(&self, id: i64, draft: ProfileDraft) -> Result<Option<Profile>> {
        let profile = self.store.update_profile(id, &draft)?;
        if profile.is_some() {
            self.events.emit(RemoteRuntimeEvent::ProfilesChanged);
        }
        Ok(profile)
    }

    pub fn delete_profile(&self, id: i64) -> Result<bool> {
        let deleted = self.store.delete_profile(id)?;
        if deleted {
            self.events.emit(RemoteRuntimeEvent::ProfilesChanged);
        }
        Ok(deleted)
    }

    pub fn subscribe_events(&self) -> mpsc::Receiver<RemoteRuntimeEvent> {
        self.events.subscribe()
    }

    pub fn open_session(&self, profile_id: i64) -> Result<SessionId> {
        let profile = self
            .store
            .get_profile(profile_id)?
            .with_context(|| format!("连接配置不存在: {profile_id}"))?;
        self.store.update_last_used(profile.id)?;

        let mut file_client = create_file_client(&profile);
        let connection = file_client.connect()?;
        let remote_root = normalize_remote_dir(&profile.paths.remote_root, profile.protocol);
        let remote_entries = file_client.list(&remote_root).unwrap_or_default();
        let session_id = SessionId::new();

        let mut session_status =
            if profile.protocol.supports_file_browser() && remote_entries.is_empty() {
                SessionStatus::Degraded
            } else {
                SessionStatus::Connected
            };
        let mut session_message = connection.message.clone();

        let terminal = if profile.protocol.supports_terminal() {
            match LiveTerminal::spawn(profile.clone(), session_id.clone(), self.events.clone()) {
                Ok(terminal) => Some(terminal),
                Err(error) => {
                    session_status = SessionStatus::Degraded;
                    session_message = format!("{}；终端启动失败: {error}", connection.message);
                    None
                }
            }
        } else {
            None
        };

        let summary = SessionSummary {
            session_id: session_id.clone(),
            profile_id: profile.id,
            title: format!("{} · {}", profile.name, profile.protocol.label()),
            protocol: profile.protocol,
            endpoint: profile.endpoint(),
            status: session_status,
            has_terminal: profile.protocol.supports_terminal(),
            transfer_count: 0,
            message: session_message,
        };
        let snapshot = SessionSnapshot {
            remote_root: remote_root.clone(),
            remote_entries,
            local_root: expand_local_root(&profile.paths.local_root)
                .to_string_lossy()
                .to_string(),
            editable_files: Vec::new(),
            summary,
            connection,
        };

        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow!("session manager lock poisoned"))?;
        let transfer_limit = profile.limits.transfer_concurrency as usize;
        sessions.insert(
            session_id.clone(),
            SessionRuntime {
                profile,
                snapshot,
                terminal,
            },
        );
        self.transfer_queue
            .set_session_limit(session_id.clone(), transfer_limit);
        self.events.emit(RemoteRuntimeEvent::ProfilesChanged);
        self.events.emit(RemoteRuntimeEvent::SessionsChanged);
        self.events
            .emit(RemoteRuntimeEvent::SessionChanged(session_id.clone()));
        Ok(session_id)
    }

    pub fn close_session(&self, session_id: &SessionId) -> Result<bool> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow!("session manager lock poisoned"))?;
        let removed = sessions.remove(session_id).is_some();
        if removed {
            self.transfer_queue.remove_session_limit(session_id);
            self.events.emit(RemoteRuntimeEvent::SessionsChanged);
        }
        Ok(removed)
    }

    pub fn refresh_session_directory(
        &self,
        session_id: &SessionId,
        path: Option<&str>,
    ) -> Result<Vec<RemoteEntry>> {
        let (profile, remote_root) = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| anyhow!("session manager lock poisoned"))?;
            let session = sessions
                .get(session_id)
                .with_context(|| format!("session 不存在: {}", session_id.0))?;
            let remote_root = path
                .map(|p| normalize_remote_dir(p, session.profile.protocol))
                .unwrap_or_else(|| session.snapshot.remote_root.clone());
            (session.profile.clone(), remote_root)
        };

        let file_client = create_file_client(&profile);
        let remote_entries = file_client.list(&remote_root)?;
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow!("session manager lock poisoned"))?;
        let session = sessions
            .get_mut(session_id)
            .with_context(|| format!("session 不存在: {}", session_id.0))?;
        session.snapshot.remote_root = remote_root;
        session.snapshot.remote_entries = remote_entries.clone();
        session.snapshot.summary.status = SessionStatus::Connected;
        self.events
            .emit(RemoteRuntimeEvent::SessionChanged(session_id.clone()));
        Ok(remote_entries)
    }

    pub fn enter_directory(&self, session_id: &SessionId, remote_path: &str) -> Result<()> {
        self.refresh_session_directory(session_id, Some(remote_path))?;
        Ok(())
    }

    pub fn parent_directory(&self, session_id: &SessionId) -> Result<()> {
        let current = self
            .session_snapshot(session_id)
            .map(|snapshot| snapshot.remote_root)
            .with_context(|| format!("session 不存在: {}", session_id.0))?;
        let parent = remote_parent_dir(&current);
        self.refresh_session_directory(session_id, Some(&parent))?;
        Ok(())
    }

    pub fn create_remote_directory(&self, session_id: &SessionId, path: &str) -> Result<()> {
        let (profile, remote_root) = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| anyhow!("session manager lock poisoned"))?;
            let session = sessions
                .get(session_id)
                .with_context(|| format!("session 不存在: {}", session_id.0))?;
            (
                session.profile.clone(),
                session.snapshot.remote_root.clone(),
            )
        };

        create_file_client(&profile).mkdir(path)?;
        self.refresh_session_directory(session_id, Some(&remote_root))?;
        Ok(())
    }

    pub fn rename_remote_entry(&self, session_id: &SessionId, from: &str, to: &str) -> Result<()> {
        let (profile, remote_root) = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| anyhow!("session manager lock poisoned"))?;
            let session = sessions
                .get(session_id)
                .with_context(|| format!("session 不存在: {}", session_id.0))?;
            (
                session.profile.clone(),
                session.snapshot.remote_root.clone(),
            )
        };

        create_file_client(&profile).rename(from, to)?;
        self.refresh_session_directory(session_id, Some(&remote_root))?;
        Ok(())
    }

    pub fn remove_remote_entry(
        &self,
        session_id: &SessionId,
        path: &str,
        is_dir: bool,
    ) -> Result<()> {
        let (profile, remote_root) = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| anyhow!("session manager lock poisoned"))?;
            let session = sessions
                .get(session_id)
                .with_context(|| format!("session 不存在: {}", session_id.0))?;
            (
                session.profile.clone(),
                session.snapshot.remote_root.clone(),
            )
        };

        create_file_client(&profile).remove(path, is_dir)?;
        self.refresh_session_directory(session_id, Some(&remote_root))?;
        Ok(())
    }

    pub fn download_entry(
        &self,
        session_id: &SessionId,
        remote_path: &str,
        local_path: &Path,
    ) -> Result<PathBuf> {
        let (profile, remote_path_string, local_path_buf) = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| anyhow!("session manager lock poisoned"))?;
            let session = sessions
                .get(session_id)
                .with_context(|| format!("session 不存在: {}", session_id.0))?;
            (
                session.profile.clone(),
                remote_path.to_string(),
                local_path.to_path_buf(),
            )
        };

        let total_bytes = create_file_client(&profile)
            .stat(&remote_path_string)
            .map(|entry| entry.size)
            .unwrap_or_default();
        let transfer_id = self.transfer_queue.enqueue(
            session_id.clone(),
            TransferDirection::Download,
            local_path.display().to_string(),
            remote_path_string.clone(),
            total_bytes,
        );
        self.events.emit(RemoteRuntimeEvent::TransfersChanged);
        self.spawn_download_task(
            session_id.clone(),
            profile,
            transfer_id,
            remote_path_string,
            local_path_buf.clone(),
            total_bytes,
        );
        Ok(local_path_buf)
    }

    pub fn upload_file(
        &self,
        session_id: &SessionId,
        local_path: &Path,
        remote_path: &str,
    ) -> Result<()> {
        let (profile, local_path_buf, remote_path_string) = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| anyhow!("session manager lock poisoned"))?;
            let session = sessions
                .get(session_id)
                .with_context(|| format!("session 不存在: {}", session_id.0))?;
            (
                session.profile.clone(),
                local_path.to_path_buf(),
                remote_path.to_string(),
            )
        };

        let total_bytes = fs::metadata(local_path)
            .map(|meta| meta.len())
            .unwrap_or_default();
        let transfer_id = self.transfer_queue.enqueue(
            session_id.clone(),
            TransferDirection::Upload,
            local_path.display().to_string(),
            remote_path_string.clone(),
            total_bytes,
        );
        self.events.emit(RemoteRuntimeEvent::TransfersChanged);
        self.spawn_upload_task(
            session_id.clone(),
            profile,
            transfer_id,
            local_path_buf,
            remote_path_string,
            total_bytes,
        );
        Ok(())
    }

    pub fn download_for_edit(&self, session_id: &SessionId, remote_path: &str) -> Result<PathBuf> {
        let path = self.edit_cache_path(session_id, remote_path)?;
        let profile = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| anyhow!("session manager lock poisoned"))?;
            let session = sessions
                .get(session_id)
                .with_context(|| format!("session 不存在: {}", session_id.0))?;
            session.profile.clone()
        };
        self.download_entry_blocking(session_id, profile, remote_path, &path)?;

        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow!("session manager lock poisoned"))?;
        let session = sessions
            .get_mut(session_id)
            .with_context(|| format!("session 不存在: {}", session_id.0))?;
        if session
            .snapshot
            .editable_files
            .iter()
            .all(|item| item.remote_path != remote_path)
        {
            session.snapshot.editable_files.push(EditableFile {
                local_path: path.clone(),
                remote_path: remote_path.to_string(),
            });
        }
        self.events
            .emit(RemoteRuntimeEvent::SessionChanged(session_id.clone()));
        Ok(path)
    }

    pub fn upload_edited_file(
        &self,
        session_id: &SessionId,
        local_path: &Path,
        remote_path: &str,
    ) -> Result<()> {
        let profile = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| anyhow!("session manager lock poisoned"))?;
            let session = sessions
                .get(session_id)
                .with_context(|| format!("session 不存在: {}", session_id.0))?;
            session.profile.clone()
        };
        self.upload_file_blocking(session_id, profile, local_path, remote_path)
    }

    pub fn default_download_path(
        &self,
        session_id: &SessionId,
        remote_path: &str,
    ) -> Result<PathBuf> {
        let snapshot = self
            .session_snapshot(session_id)
            .with_context(|| format!("session 不存在: {}", session_id.0))?;
        let name = remote_name(remote_path);
        let root = PathBuf::from(snapshot.local_root);
        fs::create_dir_all(&root)
            .with_context(|| format!("创建下载目录失败: {}", root.display()))?;
        Ok(root.join(name))
    }

    pub fn send_terminal_input(&self, session_id: &SessionId, input: TerminalInput) -> Result<()> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow!("session manager lock poisoned"))?;
        let session = sessions
            .get(session_id)
            .with_context(|| format!("session 不存在: {}", session_id.0))?;
        let terminal = session
            .terminal
            .as_ref()
            .context("当前 session 没有可用终端")?;
        terminal.send_input(input)
    }

    pub fn resize_terminal(&self, session_id: &SessionId, columns: u32, rows: u32) -> Result<()> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow!("session manager lock poisoned"))?;
        let session = sessions
            .get(session_id)
            .with_context(|| format!("session 不存在: {}", session_id.0))?;
        let terminal = session
            .terminal
            .as_ref()
            .context("当前 session 没有可用终端")?;
        terminal.resize(columns, rows)?;
        self.events
            .emit(RemoteRuntimeEvent::TerminalChanged(session_id.clone()));
        Ok(())
    }

    pub fn has_live_session(&self, profile_id: i64) -> bool {
        self.sessions
            .lock()
            .map(|sessions| {
                sessions
                    .values()
                    .any(|session| session.snapshot.summary.profile_id == profile_id)
            })
            .unwrap_or(false)
    }

    pub fn session_summaries(&self) -> Vec<SessionSummary> {
        let mut items: Vec<SessionSummary> = self
            .sessions
            .lock()
            .map(|mut sessions| {
                sessions
                    .values_mut()
                    .map(|session| {
                        sync_summary_counts(session, &self.transfer_queue);
                        let mut summary = session.snapshot.summary.clone();
                        summary.transfer_count = self
                            .transfer_queue
                            .snapshots_for_session(&summary.session_id)
                            .len();
                        summary
                    })
                    .collect()
            })
            .unwrap_or_default();
        items.sort_by(|a, b| a.title.cmp(&b.title));
        items
    }

    pub fn session_snapshot(&self, session_id: &SessionId) -> Option<SessionSnapshot> {
        self.sessions
            .lock()
            .ok()
            .and_then(|mut sessions| {
                let session = sessions.get_mut(session_id)?;
                sync_summary_counts(session, &self.transfer_queue);
                Some(session.snapshot.clone())
            })
            .map(|mut snapshot| {
                snapshot.summary.transfer_count = self
                    .transfer_queue
                    .snapshots_for_session(&snapshot.summary.session_id)
                    .len();
                snapshot
            })
    }

    pub fn terminal_frame(&self, session_id: &SessionId) -> Option<TerminalFrame> {
        let sessions = self.sessions.lock().ok()?;
        let session = sessions.get(session_id)?;
        session.terminal.as_ref().and_then(LiveTerminal::frame)
    }

    pub fn terminal_revision(&self, session_id: &SessionId) -> Option<u64> {
        let sessions = self.sessions.lock().ok()?;
        let session = sessions.get(session_id)?;
        session.terminal.as_ref().and_then(LiveTerminal::revision)
    }

    pub fn all_transfer_snapshots(&self) -> Vec<TransferSnapshot> {
        self.transfer_queue.all_snapshots()
    }

    pub fn cancel_transfer(&self, transfer_id: &crate::model::TransferId) {
        self.transfer_queue.cancel(transfer_id);
        self.events.emit(RemoteRuntimeEvent::TransfersChanged);
    }

    fn download_entry_blocking(
        &self,
        session_id: &SessionId,
        profile: Profile,
        remote_path: &str,
        local_path: &Path,
    ) -> Result<PathBuf> {
        let remote_path_string = remote_path.to_string();
        let total_bytes = create_file_client(&profile)
            .stat(&remote_path_string)
            .map(|entry| entry.size)
            .unwrap_or_default();
        let transfer_id = self.transfer_queue.enqueue(
            session_id.clone(),
            TransferDirection::Download,
            local_path.display().to_string(),
            remote_path_string.clone(),
            total_bytes,
        );
        self.events.emit(RemoteRuntimeEvent::TransfersChanged);
        if !self.transfer_queue.wait_and_start(&transfer_id) {
            self.transfer_queue
                .finish(&transfer_id, false, String::from("已取消"));
            self.events.emit(RemoteRuntimeEvent::TransfersChanged);
            return Err(anyhow!("下载已取消"));
        }
        self.events.emit(RemoteRuntimeEvent::TransfersChanged);

        let result = create_file_client(&profile)
            .download(&remote_path_string, &local_path.display().to_string());
        match result {
            Ok(()) => {
                let written = fs::metadata(local_path)
                    .map(|meta| meta.len())
                    .unwrap_or(total_bytes);
                self.transfer_queue
                    .mark_progress(&transfer_id, written, Some(written));
                self.transfer_queue
                    .finish(&transfer_id, true, String::from("下载完成"));
                self.events.emit(RemoteRuntimeEvent::TransfersChanged);
                Ok(local_path.to_path_buf())
            }
            Err(error) => {
                self.transfer_queue
                    .finish(&transfer_id, false, format!("下载失败: {error}"));
                self.events.emit(RemoteRuntimeEvent::TransfersChanged);
                Err(error)
            }
        }
    }

    fn upload_file_blocking(
        &self,
        session_id: &SessionId,
        profile: Profile,
        local_path: &Path,
        remote_path: &str,
    ) -> Result<()> {
        let total_bytes = fs::metadata(local_path)
            .map(|meta| meta.len())
            .unwrap_or_default();
        let transfer_id = self.transfer_queue.enqueue(
            session_id.clone(),
            TransferDirection::Upload,
            local_path.display().to_string(),
            remote_path.to_string(),
            total_bytes,
        );
        self.events.emit(RemoteRuntimeEvent::TransfersChanged);
        if !self.transfer_queue.wait_and_start(&transfer_id) {
            self.transfer_queue
                .finish(&transfer_id, false, String::from("已取消"));
            self.events.emit(RemoteRuntimeEvent::TransfersChanged);
            return Err(anyhow!("上传已取消"));
        }
        self.events.emit(RemoteRuntimeEvent::TransfersChanged);

        let result =
            create_file_client(&profile).upload(&local_path.display().to_string(), remote_path);
        match result {
            Ok(()) => {
                self.transfer_queue
                    .mark_progress(&transfer_id, total_bytes, Some(total_bytes));
                self.transfer_queue
                    .finish(&transfer_id, true, String::from("上传完成"));
                self.events.emit(RemoteRuntimeEvent::TransfersChanged);
                self.refresh_session_directory(session_id, None)?;
                Ok(())
            }
            Err(error) => {
                self.transfer_queue
                    .finish(&transfer_id, false, format!("上传失败: {error}"));
                self.events.emit(RemoteRuntimeEvent::TransfersChanged);
                Err(error)
            }
        }
    }

    fn spawn_download_task(
        &self,
        _session_id: SessionId,
        profile: Profile,
        transfer_id: crate::model::TransferId,
        remote_path: String,
        local_path: PathBuf,
        total_bytes: u64,
    ) {
        let queue = Arc::clone(&self.transfer_queue);
        let events = self.events.clone();
        thread::spawn(move || {
            if !queue.wait_and_start(&transfer_id) {
                queue.finish(&transfer_id, false, String::from("已取消"));
                events.emit(RemoteRuntimeEvent::TransfersChanged);
                return;
            }
            events.emit(RemoteRuntimeEvent::TransfersChanged);
            let result = create_file_client(&profile)
                .download(&remote_path, &local_path.display().to_string());
            match result {
                Ok(()) => {
                    let written = fs::metadata(&local_path)
                        .map(|meta| meta.len())
                        .unwrap_or(total_bytes);
                    queue.mark_progress(&transfer_id, written, Some(written));
                    queue.finish(&transfer_id, true, String::from("下载完成"));
                    events.emit(RemoteRuntimeEvent::TransfersChanged);
                }
                Err(error) => {
                    queue.finish(&transfer_id, false, format!("下载失败: {error}"));
                    events.emit(RemoteRuntimeEvent::TransfersChanged);
                }
            }
        });
    }

    fn spawn_upload_task(
        &self,
        session_id: SessionId,
        profile: Profile,
        transfer_id: crate::model::TransferId,
        local_path: PathBuf,
        remote_path: String,
        total_bytes: u64,
    ) {
        let sessions = Arc::clone(&self.sessions);
        let queue = Arc::clone(&self.transfer_queue);
        let events = self.events.clone();
        thread::spawn(move || {
            if !queue.wait_and_start(&transfer_id) {
                queue.finish(&transfer_id, false, String::from("已取消"));
                events.emit(RemoteRuntimeEvent::TransfersChanged);
                return;
            }
            events.emit(RemoteRuntimeEvent::TransfersChanged);
            let result = create_file_client(&profile)
                .upload(&local_path.display().to_string(), &remote_path);
            match result {
                Ok(()) => {
                    queue.mark_progress(&transfer_id, total_bytes, Some(total_bytes));
                    queue.finish(&transfer_id, true, String::from("上传完成"));
                    events.emit(RemoteRuntimeEvent::TransfersChanged);
                    let refresh_target = sessions.lock().ok().and_then(|sessions| {
                        let session = sessions.get(&session_id)?;
                        Some((
                            session.profile.clone(),
                            session.snapshot.remote_root.clone(),
                        ))
                    });
                    if let Some((profile, root)) = refresh_target
                        && let Ok(entries) = create_file_client(&profile).list(&root)
                        && let Ok(mut sessions) = sessions.lock()
                        && let Some(session) = sessions.get_mut(&session_id)
                    {
                        session.snapshot.remote_entries = entries;
                        session.snapshot.summary.status = SessionStatus::Connected;
                        events.emit(RemoteRuntimeEvent::SessionChanged(session_id.clone()));
                    }
                }
                Err(error) => {
                    queue.finish(&transfer_id, false, format!("上传失败: {error}"));
                    events.emit(RemoteRuntimeEvent::TransfersChanged);
                }
            }
        });
    }

    fn edit_cache_path(&self, session_id: &SessionId, remote_path: &str) -> Result<PathBuf> {
        let dir = self
            .paths
            .feature_dir(PLUGIN_ID)
            .join("editable")
            .join(session_id.0.to_string());
        fs::create_dir_all(&dir)
            .with_context(|| format!("创建编辑缓存目录失败: {}", dir.display()))?;
        Ok(dir.join(remote_name(remote_path)))
    }
}

fn sync_summary_counts(session: &mut SessionRuntime, transfer_queue: &TransferQueue) {
    session.snapshot.summary.transfer_count = transfer_queue
        .snapshots_for_session(&session.snapshot.summary.session_id)
        .len();
}

fn spawn_terminal_thread(
    profile: Profile,
    session_id: SessionId,
    engine: Arc<Mutex<TerminalEngine>>,
    events: RuntimeEventBus,
) -> Result<tokio_mpsc::UnboundedSender<TerminalCommand>> {
    let (command_tx, mut command_rx) = tokio_mpsc::unbounded_channel();
    let thread_name = format!("qingqi-ssh-terminal-{}", profile.id);
    thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    if let Ok(mut terminal) = engine.lock() {
                        let _ = terminal.feed_bytes(
                            format!("\r\n[terminal runtime error] {error}\r\n").as_bytes(),
                        );
                    }
                    events.emit(RemoteRuntimeEvent::TerminalChanged(session_id.clone()));
                    return;
                }
            };

            runtime.block_on(async move {
                let connection = match connect_ssh(profile.clone()).await {
                    Ok(connection) => connection,
                    Err(error) => {
                        if let Ok(mut terminal) = engine.lock() {
                            let _ = terminal.feed_bytes(
                                format!("\r\n[terminal connect error] {error}\r\n").as_bytes(),
                            );
                        }
                        events.emit(RemoteRuntimeEvent::TerminalChanged(session_id.clone()));
                        return;
                    }
                };

                let mut channel = match connection.open_terminal_channel(96, 28).await {
                    Ok(channel) => channel,
                    Err(error) => {
                        if let Ok(mut terminal) = engine.lock() {
                            let _ = terminal.feed_bytes(
                                format!("\r\n[terminal open error] {error}\r\n").as_bytes(),
                            );
                        }
                        events.emit(RemoteRuntimeEvent::TerminalChanged(session_id.clone()));
                        return;
                    }
                };

                if let Ok(mut terminal) = engine.lock() {
                    let _ = terminal.feed_bytes(
                        format!(
                            "\r\n\x1b[90mSSH shell attached: {}\x1b[0m\r\n",
                            profile.endpoint()
                        )
                        .as_bytes(),
                    );
                }
                events.emit(RemoteRuntimeEvent::TerminalChanged(session_id.clone()));

                loop {
                    tokio::select! {
                        biased;
                        command = command_rx.recv() => match command {
                            Some(TerminalCommand::Input(bytes)) => {
                                if let Err(error) = channel.send_input(&bytes).await
                                    && let Ok(mut terminal) = engine.lock()
                                {
                                    let _ = terminal.feed_bytes(
                                        format!("\r\n[input error] {error}\r\n").as_bytes(),
                                    );
                                    events.emit(RemoteRuntimeEvent::TerminalChanged(
                                        session_id.clone(),
                                    ));
                                }
                            }
                            Some(TerminalCommand::Resize { columns, rows }) => {
                                if let Err(error) = channel.resize(columns, rows).await
                                    && let Ok(mut terminal) = engine.lock()
                                {
                                    let _ = terminal.feed_bytes(
                                        format!("\r\n[resize error] {error}\r\n").as_bytes(),
                                    );
                                    events.emit(RemoteRuntimeEvent::TerminalChanged(
                                        session_id.clone(),
                                    ));
                                }
                            }
                            Some(TerminalCommand::Shutdown) | None => {
                                let _ = channel.close().await;
                                return;
                            }
                        },
                        message = channel.recv() => match message {
                            Some(ChannelMsg::Data { data }) => {
                                if let Ok(mut terminal) = engine.lock() {
                                    let _ = terminal.feed_bytes(data.as_ref());
                                }
                                events.emit(RemoteRuntimeEvent::TerminalChanged(session_id.clone()));
                            }
                            Some(ChannelMsg::ExtendedData { data, .. }) => {
                                if let Ok(mut terminal) = engine.lock() {
                                    let _ = terminal.feed_bytes(data.as_ref());
                                }
                                events.emit(RemoteRuntimeEvent::TerminalChanged(session_id.clone()));
                            }
                            Some(ChannelMsg::ExitStatus { exit_status }) => {
                                if let Ok(mut terminal) = engine.lock() {
                                    let _ = terminal.feed_bytes(
                                        format!(
                                            "\r\n\x1b[90m[process exited: {}]\x1b[0m\r\n",
                                            exit_status
                                        )
                                        .as_bytes(),
                                    );
                                }
                                events.emit(RemoteRuntimeEvent::TerminalChanged(session_id.clone()));
                                return;
                            }
                            Some(ChannelMsg::Eof | ChannelMsg::Close) => return,
                            Some(_) => {}
                            None => return,
                        },
                    }
                }
            });
        })
        .context("启动 SSH terminal 后台线程失败")?;
    Ok(command_tx)
}

fn expand_local_root(value: &str) -> PathBuf {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return default_downloads_dir();
    }
    if trimmed == "~" {
        return dirs::home_dir().unwrap_or_else(default_downloads_dir);
    }
    if let Some(rest) = trimmed.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(trimmed)
}

fn default_downloads_dir() -> PathBuf {
    dirs::download_dir().unwrap_or_else(std::env::temp_dir)
}

fn normalize_remote_dir(path: &str, protocol: RemoteProtocol) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "." {
        return protocol.default_remote_root().to_string();
    }
    if trimmed == "~" || trimmed.starts_with("~/") {
        trimmed.trim_end_matches('/').to_string().if_empty("~")
    } else if trimmed.starts_with('/') {
        trimmed.trim_end_matches('/').to_string().if_empty("/")
    } else {
        format!("/{}", trimmed.trim_end_matches('/'))
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

fn remote_parent_dir(path: &str) -> String {
    let current = normalize_remote_dir(path, RemoteProtocol::Ssh);
    if current == "~" {
        return current;
    }
    if let Some(rest) = current.strip_prefix("~/") {
        let mut segments: Vec<&str> = rest.split('/').filter(|part| !part.is_empty()).collect();
        let _ = segments.pop();
        if segments.is_empty() {
            return String::from("~");
        }
        return format!("~/{}", segments.join("/"));
    }
    if current == "/" {
        return current;
    }
    let mut segments: Vec<&str> = current.split('/').filter(|part| !part.is_empty()).collect();
    let _ = segments.pop();
    if segments.is_empty() {
        String::from("/")
    } else {
        format!("/{}", segments.join("/"))
    }
}

fn remote_name(path: &str) -> String {
    path.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("untitled")
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::{Result, anyhow};

    use super::{
        RemoteRuntime, SessionRuntime, SessionSnapshot, expand_local_root, remote_parent_dir,
    };
    use crate::{
        model::{
            AuthConfig, ConnectionLimits, Profile, ProfilePaths, RemoteProtocol, SecurityPolicy,
            SessionId, SessionStatus, SessionSummary,
        },
        protocols::ConnectionHealth,
    };
    use qingqi_plugin::{database::DatabaseService, storage::AppPaths};

    fn make_runtime(label: &str) -> Result<RemoteRuntime> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let root = std::env::temp_dir().join(format!("qingqi-remote-runtime-{label}-{nanos}"));
        fs::create_dir_all(&root)?;
        let paths = AppPaths::for_test(root.clone());
        let database = Arc::new(DatabaseService::new(paths.clone()));
        database.register_databases(crate::databases())?;
        RemoteRuntime::new(database, paths)
    }

    fn test_profile(protocol: RemoteProtocol, name: &str) -> Profile {
        Profile {
            id: 1,
            name: name.to_string(),
            protocol,
            host: String::from("test.invalid"),
            port: protocol.default_port(),
            auth: AuthConfig::default(),
            paths: ProfilePaths::default(),
            security: SecurityPolicy::default(),
            limits: ConnectionLimits::default(),
            notes: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            last_used_at: String::new(),
        }
    }

    fn insert_fake_session(
        runtime: &RemoteRuntime,
        profile: Profile,
        title: &str,
    ) -> Result<SessionId> {
        let session_id = SessionId::new();
        let summary = SessionSummary {
            session_id: session_id.clone(),
            profile_id: profile.id,
            title: title.to_string(),
            protocol: profile.protocol,
            endpoint: profile.endpoint(),
            status: SessionStatus::Connected,
            has_terminal: profile.protocol.supports_terminal(),
            transfer_count: 0,
            message: String::from("test session"),
        };
        let snapshot = SessionSnapshot {
            summary,
            connection: ConnectionHealth {
                protocol: profile.protocol,
                can_terminal: profile.protocol.supports_terminal(),
                can_files: true,
                message: String::from("ok"),
            },
            remote_root: String::from("/"),
            remote_entries: Vec::new(),
            local_root: std::env::temp_dir().display().to_string(),
            editable_files: Vec::new(),
        };
        let runtime_session = SessionRuntime {
            profile,
            snapshot,
            terminal: None,
        };
        let mut sessions = runtime
            .sessions
            .lock()
            .map_err(|_| anyhow!("session manager lock poisoned"))?;
        if sessions.is_empty() {
            *sessions = HashMap::new();
        }
        sessions.insert(session_id.clone(), runtime_session);
        Ok(session_id)
    }

    #[test]
    fn opens_multiple_sessions_for_same_profile() -> Result<()> {
        let runtime = make_runtime("multi-session")?;
        let profile = test_profile(RemoteProtocol::Ftp, "multi");
        let first = insert_fake_session(&runtime, profile.clone(), "multi-1")?;
        let second = insert_fake_session(&runtime, profile, "multi-2")?;
        assert_ne!(first, second);
        assert_eq!(runtime.session_summaries().len(), 2);
        Ok(())
    }

    #[test]
    fn closing_one_session_keeps_others() -> Result<()> {
        let runtime = make_runtime("close-one")?;
        let profile = test_profile(RemoteProtocol::Ftp, "close");
        let first = insert_fake_session(&runtime, profile.clone(), "close-1")?;
        let second = insert_fake_session(&runtime, profile, "close-2")?;
        assert!(runtime.close_session(&first)?);
        let summaries = runtime.session_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].session_id, second);
        Ok(())
    }

    #[test]
    fn resolves_parent_directory() {
        assert_eq!(remote_parent_dir("/"), "/");
        assert_eq!(remote_parent_dir("/var/log"), "/var");
    }

    #[test]
    fn expands_tilde_path() {
        let expanded = expand_local_root("~/Downloads");
        assert!(expanded.is_absolute());
    }

    #[test]
    fn insert_fake_session_records_editable_files() -> Result<()> {
        let runtime = make_runtime("editable-record")?;
        let profile = test_profile(RemoteProtocol::Ftp, "editable");
        let session_id = insert_fake_session(&runtime, profile, "editable-1")?;

        {
            let mut sessions = runtime
                .sessions
                .lock()
                .map_err(|_| anyhow!("session manager lock poisoned"))?;
            let session = sessions
                .get_mut(&session_id)
                .ok_or_else(|| anyhow!("missing fake session"))?;
            session.snapshot.editable_files.push(super::EditableFile {
                local_path: std::env::temp_dir().join("editable.txt"),
                remote_path: String::from("/editable.txt"),
            });
        }

        let snapshot = runtime
            .session_snapshot(&session_id)
            .ok_or_else(|| anyhow!("missing snapshot"))?;
        assert_eq!(snapshot.editable_files.len(), 1);
        assert_eq!(snapshot.editable_files[0].remote_path, "/editable.txt");
        Ok(())
    }
}

//! 核心服务 — 组装所有子模块

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use tokio::sync::broadcast;
use tracing::{debug, warn};

use crate::connection::ConnectionPool;
use crate::model::{
    Profile, ProfileDraft, ProtocolType, RemoteEntry, SessionId, SessionSnapshot, SessionStatus,
    SessionSummary, SshRole, SshSnapshot, TerminalKind, TransferDirection,
};
use crate::protocol::LogLevel;
use crate::store::ProfileStore;
use crate::terminal::{TerminalEngine, TerminalFrame};
use crate::transfer::TransferQueue;

// ============ 事件 ============

#[derive(Clone, Debug)]
pub enum SshEvent {
    ProfileCreated(i64),
    ProfileUpdated(i64),
    ProfileDeleted(i64),
    SessionOpened(SessionId),
    SessionConnected(SessionId),
    SessionDisconnected(SessionId),
    SessionDataChanged(SessionId),
    SessionClosed(SessionId),
    TransferChanged(SessionId, crate::model::TransferId),
}

// ============ Session 内部状态 ============

struct SessionState {
    profile_id: i64,
    summary: SessionSummary,
    terminal_protocol: Option<Arc<dyn crate::protocol::RemoteProtocol>>,
    sftp_protocol: Option<Arc<dyn crate::protocol::RemoteProtocol>>,
    terminal: Option<Arc<TerminalEngine>>,
    entries: Vec<RemoteEntry>,
    remote_cwd: String,
    shell_cwd: Option<String>,
    transfer_queue: Arc<TransferQueue>,
}

// ============ SshService ============

pub struct SshService {
    profile_store: Arc<ProfileStore>,
    cache_dir: PathBuf,
    connection_pool: Arc<ConnectionPool>,
    sessions: Arc<Mutex<HashMap<SessionId, SessionState>>>,
    event_tx: broadcast::Sender<SshEvent>,
    revision: AtomicU64,
}

impl SshService {
    pub fn new(
        _database: Arc<qingqi_plugin::database::DatabaseService>,
        profile_store: Arc<ProfileStore>,
        cache_dir: PathBuf,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        let (disconnect_tx, _) = broadcast::channel(64);
        let sessions = Arc::new(Mutex::new(HashMap::new()));
        let connection_pool = Arc::new(ConnectionPool::new(disconnect_tx.clone()));

        let sessions_listener = Arc::clone(&sessions);
        let event_tx_listener = event_tx.clone();
        let mut disconnect_rx = disconnect_tx.subscribe();
        crate::tokio_handle().spawn(async move {
            loop {
                match disconnect_rx.recv().await {
                    Ok((profile_id, role)) => {
                        Self::mark_sessions_disconnected(
                            &sessions_listener,
                            &event_tx_listener,
                            profile_id,
                            role,
                        );
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Self {
            profile_store,
            cache_dir,
            connection_pool,
            sessions,
            event_tx,
            revision: AtomicU64::new(0),
        }
    }

    fn mark_sessions_disconnected(
        sessions: &Arc<Mutex<HashMap<SessionId, SessionState>>>,
        event_tx: &broadcast::Sender<SshEvent>,
        profile_id: i64,
        role: SshRole,
    ) {
        let message = match role {
            SshRole::Terminal => "终端连接已断开",
            SshRole::Sftp => "SFTP 连接已断开",
        };
        let mut affected = Vec::new();
        let mut guard = sessions.lock().unwrap_or_else(|e| e.into_inner());
        for state in guard.values_mut() {
            if state.profile_id != profile_id {
                continue;
            }
            if !matches!(
                state.summary.status,
                SessionStatus::Connected | SessionStatus::Connecting
            ) {
                continue;
            }
            match role {
                SshRole::Terminal => state.terminal_protocol = None,
                SshRole::Sftp => state.sftp_protocol = None,
            }
            state.summary.status = SessionStatus::Disconnected;
            state.summary.message = message.into();
            affected.push(state.summary.session_id);
        }
        drop(guard);
        for sid in affected {
            Self::notify_async(SshEvent::SessionDisconnected(sid), event_tx);
        }
    }

    fn bump(&self) {
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    fn emit(&self, event: SshEvent) {
        self.bump();
        debug!(target: "qingqi_ssh", ?event, revision = self.revision.load(Ordering::SeqCst), "service: emit");
        let _ = self.event_tx.send(event);
    }

    /// 异步任务内通知 UI（不 bump revision，View 已不再依赖 revision）
    fn notify_async(event: SshEvent, tx: &broadcast::Sender<SshEvent>) {
        debug!(target: "qingqi_ssh", ?event, "service: notify_async");
        let _ = tx.send(event);
    }

    fn patch_session(
        sessions: &Arc<Mutex<HashMap<SessionId, SessionState>>>,
        sid: SessionId,
        patch: impl FnOnce(&mut SessionState),
    ) {
        let mut guard = sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = guard.get_mut(&sid) {
            patch(state);
        }
    }

    // ========== Profile CRUD ==========

    pub fn list_profiles(&self) -> Result<Vec<Profile>> {
        self.profile_store.list()
    }

    pub fn get_profile(&self, id: i64) -> Result<Option<Profile>> {
        self.profile_store.get(id)
    }

    pub fn create_profile(&self, draft: ProfileDraft) -> Result<Profile> {
        let profile = self.profile_store.create(&draft)?;
        self.emit(SshEvent::ProfileCreated(profile.id));
        Ok(profile)
    }

    pub fn update_profile(&self, id: i64, draft: ProfileDraft) -> Result<Profile> {
        let profile = self
            .profile_store
            .update(id, &draft)?
            .ok_or_else(|| anyhow::anyhow!("Profile {id} 不存在"))?;
        self.emit(SshEvent::ProfileUpdated(id));
        Ok(profile)
    }

    pub fn delete_profile(&self, id: i64) -> Result<bool> {
        let deleted = self.profile_store.delete(id)?;
        if deleted {
            self.emit(SshEvent::ProfileDeleted(id));
        }
        Ok(deleted)
    }

    // ========== Session 管理 ==========

    pub fn open_session(&self, profile_id: i64) -> Result<SessionId> {
        debug!(
            target: "qingqi_ssh",
            profile_id,
            "open_session: 加载 Profile"
        );
        let profile = self
            .get_profile(profile_id)?
            .ok_or_else(|| anyhow::anyhow!("Profile {profile_id} 不存在"))?;

        debug!(
            target: "qingqi_ssh",
            profile_id,
            name = %profile.name,
            protocol = %profile.protocol.display(),
            host = %profile.host,
            port = profile.port,
            remote_root = %profile.paths.remote_root,
            timeout_secs = profile.advanced.connection_timeout_secs,
            keepalive_secs = profile.advanced.keepalive_interval_secs,
            "open_session: Profile 已加载，创建 Session"
        );

        let session_id = SessionId::new();
        let terminal_kind = profile.protocol.supports_terminal();

        let summary = SessionSummary {
            session_id,
            profile_id,
            title: profile.name.clone(),
            endpoint: format!("{}:{}", profile.host, profile.port),
            protocol: profile.protocol.clone(),
            status: SessionStatus::Connecting,
            terminal_kind: terminal_kind.clone(),
            has_terminal: true,
            message: "连接中...".into(),
        };

        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.insert(
            session_id,
            SessionState {
                profile_id,
                summary: summary.clone(),
                terminal_protocol: None,
                sftp_protocol: None,
                terminal: None,
                entries: Vec::new(),
                remote_cwd: profile.paths.remote_root.clone(),
                shell_cwd: None,
                transfer_queue: Arc::new(TransferQueue::new(session_id)),
            },
        );
        drop(sessions);

        self.emit(SshEvent::SessionOpened(session_id));
        debug!(
            target: "qingqi_ssh",
            profile_id,
            session_id = %session_id.0,
            "open_session: Session 已创建，开始异步连接"
        );

        // 异步连接
        let pool = Arc::clone(&self.connection_pool);
        let sessions = Arc::clone(&self.sessions);
        let tx = self.event_tx.clone();
        let sid = session_id;
        let p = profile;
        crate::tokio_handle().spawn(async move {
            debug!(
                target: "qingqi_ssh",
                profile_id = p.id,
                session_id = %sid.0,
                endpoint = %format!("{}:{}", p.host, p.port),
                "connect_task: 开始协议连接"
            );
            if matches!(p.protocol, ProtocolType::Ftp | ProtocolType::Ftps) {
                Self::connect_ftp_session(pool, sessions, tx, sid, p).await;
                return;
            }

            let term_result = pool.get_or_connect(&p, SshRole::Terminal).await;
            let sftp_result = pool.get_or_connect(&p, SshRole::Sftp).await;

            match (term_result, sftp_result) {
                (Ok(terminal_proto), Ok(sftp_proto)) => {
                    debug!(
                        target: "qingqi_ssh",
                        profile_id = p.id,
                        session_id = %sid.0,
                        "connect_task: 终端/SFTP 双连接就绪"
                    );
                    // 打开终端（独立 SSH 会话）
                    let term = match terminal_proto.open_terminal().await {
                        Ok(source) => {
                            debug!(
                                target: "qingqi_ssh",
                                profile_id = p.id,
                                session_id = %sid.0,
                                "connect_task: 终端通道已打开"
                            );
                            let engine =
                                Arc::new(TerminalEngine::new(p.protocol.supports_terminal()));
                            engine.set_status(&format!("{}@{}:{}", p.name, p.host, p.port));
                            let tx_term = tx.clone();
                            let sid_term = sid;
                            TerminalEngine::start_processing_with_notify(
                                engine.clone(),
                                source,
                                Some(Arc::new(move || {
                                    Self::notify_async(
                                        SshEvent::SessionDataChanged(sid_term),
                                        &tx_term,
                                    );
                                })),
                            );
                            let engine = Some(engine);
                            if let Some(ref term) = engine {
                                Self::patch_session(&sessions, sid, |state| {
                                    state.terminal = Some(Arc::clone(term));
                                    state.terminal_protocol =
                                        Some(Arc::clone(&terminal_proto));
                                });
                                Self::notify_async(SshEvent::SessionDataChanged(sid), &tx);
                            }
                            engine
                        }
                        Err(e) => {
                            debug!(
                                target: "qingqi_ssh",
                                profile_id = p.id,
                                session_id = %sid.0,
                                error = %e,
                                "connect_task: 打开终端失败，继续无终端模式"
                            );
                            None
                        }
                    };

                    // 拉取初始目录列表
                    let remote_root = p.paths.remote_root.clone();
                    debug!(
                        target: "qingqi_ssh",
                        profile_id = p.id,
                        session_id = %sid.0,
                        remote_root = %remote_root,
                        "connect_task: 拉取初始目录"
                    );
                    let list_result = sftp_proto.list_directory(&remote_root).await;
                    let resolved_root = sftp_proto
                        .last_list_path()
                        .unwrap_or_else(|| remote_root.clone());
                    let entries = match list_result {
                        Ok(entries) => {
                            debug!(
                                target: "qingqi_ssh",
                                profile_id = p.id,
                                session_id = %sid.0,
                                remote_root = %remote_root,
                                resolved_root = %resolved_root,
                                entry_count = entries.len(),
                                "connect_task: 初始目录加载完成"
                            );
                            entries
                        }
                        Err(e) => {
                            debug!(
                                target: "qingqi_ssh",
                                profile_id = p.id,
                                session_id = %sid.0,
                                remote_root = %remote_root,
                                error = %e,
                                "connect_task: 加载初始目录失败"
                            );
                            Vec::new()
                        }
                    };

                    // 更新 session 状态
                    let mut sessions_guard = sessions.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(state) = sessions_guard.get_mut(&sid) {
                        state.terminal_protocol = Some(terminal_proto);
                        state.sftp_protocol = Some(sftp_proto);
                        state.terminal = term;
                        state.entries = entries;
                        state.remote_cwd = resolved_root;
                        state.summary.status = SessionStatus::Connected;
                        state.summary.message = "已连接".into();
                    }
                    drop(sessions_guard);
                    debug!(
                        target: "qingqi_ssh",
                        profile_id = p.id,
                        session_id = %sid.0,
                        "connect_task: Session 已连接"
                    );
                    Self::notify_async(SshEvent::SessionConnected(sid), &tx);
                }
                (Err(e), _) | (_, Err(e)) => {
                    warn!(
                        target: "qingqi_ssh",
                        profile_id = p.id,
                        session_id = %sid.0,
                        error = %e,
                        "connect_task: 连接失败"
                    );
                    pool.disconnect_all(p.id).await;
                    let mut sessions_guard = sessions.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(state) = sessions_guard.get_mut(&sid) {
                        state.summary.status = SessionStatus::Failed;
                        state.summary.message = format!("连接失败: {e}");
                    }
                    drop(sessions_guard);
                    Self::notify_async(SshEvent::SessionDataChanged(sid), &tx);
                }
            }
        });

        Ok(session_id)
    }

    async fn connect_ftp_session(
        pool: Arc<ConnectionPool>,
        sessions: Arc<Mutex<HashMap<SessionId, SessionState>>>,
        tx: broadcast::Sender<SshEvent>,
        sid: SessionId,
        p: Profile,
    ) {
        let proto = match pool.get_connected(&p, SshRole::Sftp).await {
            Some(existing) => existing,
            None => match pool.create_arc(&p, SshRole::Sftp) {
                Ok(proto) => proto,
                Err(error) => {
                    Self::fail_session(
                        &sessions,
                        &tx,
                        sid,
                        p.id,
                        &pool,
                        format!("初始化 FTP 协议失败: {error}"),
                        None,
                    )
                    .await;
                    return;
                }
            },
        };

        let engine = Arc::new(TerminalEngine::new(TerminalKind::Log));
        engine.set_status(&format!("{}@{}:{}", p.name, p.host, p.port));

        let term = match proto.open_terminal().await {
            Ok(source) => {
                let tx_term = tx.clone();
                let sid_term = sid;
                TerminalEngine::start_processing_with_notify(
                    engine.clone(),
                    source,
                    Some(Arc::new(move || {
                        Self::notify_async(SshEvent::SessionDataChanged(sid_term), &tx_term);
                    })),
                );
                Self::patch_session(&sessions, sid, |state| {
                    state.terminal = Some(Arc::clone(&engine));
                    state.terminal_protocol = Some(Arc::clone(&proto));
                });
                Self::notify_async(SshEvent::SessionDataChanged(sid), &tx);
                Some(engine)
            }
            Err(error) => {
                warn!(
                    target: "qingqi_ssh",
                    profile_id = p.id,
                    session_id = %sid.0,
                    error = %error,
                    "connect_task: 打开 FTP 日志通道失败"
                );
                None
            }
        };

        if !proto.is_connected() {
            if let Some(engine) = term.as_ref() {
                engine.append_log(
                    LogLevel::Info,
                    &format!(
                        "suppaftp: 正在连接 {}:{} ({})…",
                        p.host,
                        p.port,
                        p.protocol.display()
                    ),
                );
                Self::notify_async(SshEvent::SessionDataChanged(sid), &tx);
            }
            if let Err(error) = proto.connect().await {
                if let Some(engine) = term.as_ref() {
                    engine.append_log(LogLevel::Error, &format!("连接失败: {error}"));
                    Self::notify_async(SshEvent::SessionDataChanged(sid), &tx);
                }
                Self::fail_session(
                    &sessions,
                    &tx,
                    sid,
                    p.id,
                    &pool,
                    format!("连接失败: {error}"),
                    term,
                )
                .await;
                return;
            }
            pool.register_connected(&p, SshRole::Sftp, Arc::clone(&proto))
                .await;
        }

        let remote_root = p.paths.remote_root.clone();
        let list_result = proto.list_directory(&remote_root).await;
        let resolved_root = proto
            .last_list_path()
            .unwrap_or_else(|| remote_root.clone());
        let entries = match list_result {
            Ok(entries) => entries,
            Err(error) => {
                if let Some(engine) = term.as_ref() {
                    engine.append_log(LogLevel::Error, &format!("加载目录失败: {error}"));
                    Self::notify_async(SshEvent::SessionDataChanged(sid), &tx);
                }
                Vec::new()
            }
        };

        let mut sessions_guard = sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = sessions_guard.get_mut(&sid) {
            state.terminal_protocol = Some(Arc::clone(&proto));
            state.sftp_protocol = Some(proto);
            state.terminal = term;
            state.entries = entries;
            state.remote_cwd = resolved_root;
            state.summary.status = SessionStatus::Connected;
            state.summary.message = "已连接".into();
        }
        drop(sessions_guard);
        Self::notify_async(SshEvent::SessionConnected(sid), &tx);
    }

    async fn fail_session(
        sessions: &Arc<Mutex<HashMap<SessionId, SessionState>>>,
        tx: &broadcast::Sender<SshEvent>,
        sid: SessionId,
        profile_id: i64,
        pool: &Arc<ConnectionPool>,
        message: String,
        terminal: Option<Arc<TerminalEngine>>,
    ) {
        warn!(
            target: "qingqi_ssh",
            profile_id,
            session_id = %sid.0,
            error = %message,
            "connect_task: 连接失败"
        );
        pool.disconnect_all(profile_id).await;
        let mut sessions_guard = sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = sessions_guard.get_mut(&sid) {
            state.summary.status = SessionStatus::Failed;
            state.summary.message = message;
            if terminal.is_some() {
                state.terminal = terminal;
            }
        }
        drop(sessions_guard);
        Self::notify_async(SshEvent::SessionDataChanged(sid), tx);
    }

    pub fn close_session(&self, id: &SessionId) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let profile_id = sessions.get(id).map(|state| state.profile_id);
        sessions.remove(id);
        let should_disconnect = profile_id.is_some_and(|pid| {
            !sessions
                .values()
                .any(|state| state.profile_id == pid)
        });
        drop(sessions);

        if should_disconnect {
            if let Some(profile_id) = profile_id {
                let pool = Arc::clone(&self.connection_pool);
                crate::tokio_handle().spawn(async move {
                    pool.disconnect_all(profile_id).await;
                });
            }
        }
        self.emit(SshEvent::SessionClosed(*id));
        Ok(())
    }

    pub fn session_summaries(&self) -> Vec<SessionSummary> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.values().map(|s| s.summary.clone()).collect()
    }

    pub fn session_summary(&self, id: &SessionId) -> Option<SessionSummary> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.get(id).map(|s| s.summary.clone())
    }

    pub fn session_snapshot(&self, id: &SessionId) -> Option<SessionSnapshot> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.get(id).map(|s| SessionSnapshot {
            summary: s.summary.clone(),
            entries: s.entries.clone(),
            remote_cwd: s.remote_cwd.clone(),
        })
    }

    pub fn snapshot(&self) -> SshSnapshot {
        let profiles = self.list_profiles().unwrap_or_default();
        let sessions = self.session_summaries();
        SshSnapshot {
            profiles,
            sessions,
            revision: self.revision.load(Ordering::SeqCst),
        }
    }

    // ========== 终端 ==========

    pub fn terminal_snapshot(&self, id: &SessionId) -> Option<TerminalFrame> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.get(id)?.terminal.as_ref().map(|t| t.snapshot())
    }

    fn terminal_protocol(&self, id: &SessionId) -> Option<Arc<dyn crate::protocol::RemoteProtocol>> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.get(id)?.terminal_protocol.as_ref().and_then(|proto| {
            proto.is_connected().then(|| Arc::clone(proto))
        })
    }

    pub fn send_terminal_input(self: &Arc<Self>, id: &SessionId, data: &[u8]) -> Result<()> {
        let data = data.to_vec();
        let sid = *id;
        let has_proto = self.terminal_protocol(&sid).is_some();
        debug!(
            target: "qingqi_ssh",
            session_id = %sid.0,
            bytes = data.len(),
            has_proto,
            "term_diag: send_terminal_input 调度"
        );
        if let Some(proto) = self.terminal_protocol(&sid) {
            crate::tokio_handle().spawn(async move {
                match proto.send_terminal_input(&data).await {
                    Ok(()) => debug!(
                        target: "qingqi_ssh",
                        session_id = %sid.0,
                        bytes = data.len(),
                        "term_diag: send_terminal_input 完成"
                    ),
                    Err(e) => warn!(
                        target: "qingqi_ssh",
                        session_id = %sid.0,
                        error = %e,
                        "term_diag: send_terminal_input 失败"
                    ),
                }
            });
            return Ok(());
        }
        let service = Arc::clone(self);
        crate::tokio_handle().spawn(async move {
            debug!(
                target: "qingqi_ssh",
                session_id = %sid.0,
                "term_diag: send_terminal_input 尝试重连"
            );
            match service.ensure_terminal_protocol(&sid).await {
                Ok(proto) => {
                    if let Err(e) = proto.send_terminal_input(&data).await {
                        warn!(target: "qingqi_ssh", error = %e, "term_diag: 重连后发送失败");
                    }
                }
                Err(e) => warn!(target: "qingqi_ssh", error = %e, "term_diag: 终端重连失败"),
            }
        });
        Ok(())
    }

    pub fn terminal_scroll(&self, id: &SessionId, delta: i32) {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = sessions.get(id) {
            if let Some(engine) = &state.terminal {
                use alacritty_terminal::grid::Scroll;
                engine.scroll_display(Scroll::Delta(delta));
            }
        }
    }

    pub fn terminal_scroll_to_bottom(&self, id: &SessionId) {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = sessions.get(id) {
            if let Some(engine) = &state.terminal {
                let offset_before = engine
                    .snapshot()
                    .display_offset;
                engine.scroll_to_bottom();
                let offset_after = engine
                    .snapshot()
                    .display_offset;
                debug!(
                    target: "qingqi_ssh",
                    session_id = %id.0,
                    offset_before,
                    offset_after,
                    "term_diag: scroll_to_bottom"
                );
            }
        }
    }

    pub fn resize_terminal(&self, id: &SessionId, cols: usize, rows: usize) -> Result<()> {
        {
            let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = sessions.get(id) {
                if let Some(engine) = &state.terminal {
                    engine.resize(cols, rows);
                }
            }
        }
        let Some(proto) = self.terminal_protocol(id) else {
            return Ok(());
        };
        let cols = cols as u16;
        let rows = rows as u16;
        crate::tokio_handle().spawn(async move {
            if let Err(e) = proto.resize_terminal(cols, rows).await {
                warn!(target: "qingqi_ssh", error = %e, "调整 PTY 尺寸失败");
            }
        });
        Ok(())
    }

    async fn ensure_sftp_protocol(
        &self,
        id: &SessionId,
    ) -> Result<Arc<dyn crate::protocol::RemoteProtocol>> {
        let (profile_id, cached) = {
            let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
            let state = sessions
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("Session 不存在"))?;
            (
                state.profile_id,
                state.sftp_protocol.clone().filter(|p| p.is_connected()),
            )
        };
        if let Some(proto) = cached {
            return Ok(proto);
        }

        let profile = self
            .get_profile(profile_id)?
            .ok_or_else(|| anyhow::anyhow!("Profile {profile_id} 不存在"))?;
        let proto = self
            .connection_pool
            .get_or_connect(&profile, SshRole::Sftp)
            .await?;
        self.restore_session_protocol(id, SshRole::Sftp, Arc::clone(&proto));
        Ok(proto)
    }

    async fn ensure_terminal_protocol(
        &self,
        id: &SessionId,
    ) -> Result<Arc<dyn crate::protocol::RemoteProtocol>> {
        let (profile_id, needs_reopen, cached) = {
            let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
            let state = sessions
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("Session 不存在"))?;
            let connected = state
                .terminal_protocol
                .as_ref()
                .is_some_and(|p| p.is_connected());
            (
                state.profile_id,
                !connected,
                state.terminal_protocol.clone(),
            )
        };
        if !needs_reopen {
            return cached.ok_or_else(|| anyhow::anyhow!("终端未连接"));
        }

        let profile = self
            .get_profile(profile_id)?
            .ok_or_else(|| anyhow::anyhow!("Profile {profile_id} 不存在"))?;
        let proto = self
            .connection_pool
            .get_or_connect(&profile, SshRole::Terminal)
            .await?;
        self.reopen_terminal_channel(id, &proto).await?;
        self.restore_session_protocol(id, SshRole::Terminal, Arc::clone(&proto));
        Ok(proto)
    }

    fn restore_session_protocol(
        &self,
        id: &SessionId,
        role: SshRole,
        proto: Arc<dyn crate::protocol::RemoteProtocol>,
    ) {
        let mut reconnected = false;
        let mut guard = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = guard.get_mut(id) {
            match role {
                SshRole::Terminal => state.terminal_protocol = Some(Arc::clone(&proto)),
                SshRole::Sftp => state.sftp_protocol = Some(proto),
            }
            if state.summary.status == SessionStatus::Disconnected {
                state.summary.status = SessionStatus::Connected;
                state.summary.message = "已连接".into();
                reconnected = true;
            }
        }
        drop(guard);
        if reconnected {
            self.emit(SshEvent::SessionConnected(*id));
        }
    }

    async fn reopen_terminal_channel(
        &self,
        id: &SessionId,
        proto: &Arc<dyn crate::protocol::RemoteProtocol>,
    ) -> Result<()> {
        let (engine, tx) = {
            let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
            let state = sessions
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("Session 不存在"))?;
            (
                state.terminal.clone(),
                self.event_tx.clone(),
            )
        };
        let Some(engine) = engine else {
            return Ok(());
        };
        let source = proto.open_terminal().await?;
        let sid = *id;
        TerminalEngine::start_processing_with_notify(
            engine,
            source,
            Some(Arc::new(move || {
                Self::notify_async(SshEvent::SessionDataChanged(sid), &tx);
            })),
        );
        Ok(())
    }

    // ========== 文件操作 ==========

    pub fn list_directory(&self, id: &SessionId, path: &str) -> Result<Vec<RemoteEntry>> {
        let proto =
            crate::tokio_handle().block_on(async { self.ensure_sftp_protocol(id).await })?;
        let entries =
            crate::tokio_handle().block_on(async { proto.list_directory(path).await })?;
        let resolved = proto.last_list_path().unwrap_or_else(|| path.to_string());

        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = sessions.get_mut(id) {
            state.entries = entries.clone();
            state.remote_cwd = resolved;
        }
        Ok(entries)
    }

    pub fn session_entries(&self, id: &SessionId) -> Vec<RemoteEntry> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions
            .get(id)
            .map(|s| s.entries.clone())
            .unwrap_or_default()
    }

    pub fn create_remote_directory(&self, id: &SessionId, path: &str) -> Result<()> {
        let proto =
            crate::tokio_handle().block_on(async { self.ensure_sftp_protocol(id).await })?;
        crate::tokio_handle().block_on(async { proto.create_directory(path).await })
    }

    pub fn remote_entry_exists(&self, id: &SessionId, remote: &str) -> Result<bool> {
        let remote = remote.trim_end_matches('/');
        let Some((parent, name)) = crate::upload::split_remote_parent(remote) else {
            return Ok(false);
        };
        let entries = self.list_directory(id, &parent)?;
        Ok(entries.iter().any(|entry| entry.name == name))
    }

    pub fn ensure_remote_directory(&self, id: &SessionId, path: &str) -> Result<()> {
        let path = path.trim_end_matches('/');
        if path.is_empty() || path == "/" {
            return Ok(());
        }
        if self.remote_entry_exists(id, path)? {
            return Ok(());
        }
        if let Some(parent) = crate::upload::remote_parent(path) {
            if parent != "/" {
                self.ensure_remote_directory(id, &parent)?;
            }
        }
        let _ = self.create_remote_directory(id, path);
        Ok(())
    }

    pub fn ensure_remote_parent_dirs(&self, id: &SessionId, remote_file: &str) -> Result<()> {
        let remote_file = remote_file.trim_end_matches('/');
        let Some(parent) = crate::upload::remote_parent(remote_file) else {
            return Ok(());
        };
        if parent.is_empty() {
            return Ok(());
        }
        self.ensure_remote_directory(id, &parent)
    }

    pub fn rename_remote_entry(&self, id: &SessionId, old_path: &str, new_path: &str) -> Result<()> {
        let proto =
            crate::tokio_handle().block_on(async { self.ensure_sftp_protocol(id).await })?;
        crate::tokio_handle().block_on(async { proto.rename_entry(old_path, new_path).await })
    }

    pub fn remove_remote_entry(&self, id: &SessionId, path: &str, is_dir: bool) -> Result<()> {
        let proto =
            crate::tokio_handle().block_on(async { self.ensure_sftp_protocol(id).await })?;
        crate::tokio_handle().block_on(async {
            if is_dir {
                proto.remove_directory(path).await
            } else {
                proto.remove_file(path).await
            }
        })
    }

    /// 读取远程文本文件，限制 512 KB。
    pub fn read_remote_file(&self, id: &SessionId, path: &str) -> Result<String> {
        const MAX_BYTES: usize = 512 * 1024;
        let proto =
            crate::tokio_handle().block_on(async { self.ensure_sftp_protocol(id).await })?;
        let data =
            crate::tokio_handle().block_on(async { proto.read_file(path).await })?;
        if data.len() > MAX_BYTES {
            anyhow::bail!("文件过大（{}），暂不支持在线编辑", crate::transfer::format_size(data.len() as u64));
        }
        String::from_utf8(data).map_err(|_| anyhow::anyhow!("文件不是有效的 UTF-8 文本"))
    }

    pub fn write_remote_file(&self, id: &SessionId, path: &str, content: &str) -> Result<()> {
        let proto =
            crate::tokio_handle().block_on(async { self.ensure_sftp_protocol(id).await })?;
        crate::tokio_handle().block_on(async { proto.write_file(path, content.as_bytes()).await })
    }

    /// 本地临时编辑目录：`{cache_dir}/edit/{session_id}/{file_name}`
    pub fn edit_temp_path(&self, id: &SessionId, file_name: &str) -> PathBuf {
        self.cache_dir
            .join("edit")
            .join(id.0.to_string())
            .join(file_name)
    }

    /// 同步下载到本地（不走传输队列，供系统编辑器打开）。
    pub fn download_file_local(
        &self,
        id: &SessionId,
        remote: &str,
        local: &std::path::Path,
    ) -> Result<()> {
        if let Some(parent) = local.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建目录 {} 失败", parent.display()))?;
        }
        let proto =
            crate::tokio_handle().block_on(async { self.ensure_sftp_protocol(id).await })?;
        let (tx, _rx) =
            tokio::sync::mpsc::unbounded_channel::<crate::protocol::TransferProgress>();
        crate::tokio_handle().block_on(async { proto.download_file(remote, local, tx).await })
    }

    fn session_transfer_context(
        &self,
        id: &SessionId,
    ) -> Result<(SessionId, Arc<TransferQueue>, broadcast::Sender<SshEvent>)> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let state = sessions
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Session 不存在"))?;
        Ok((
            *id,
            Arc::clone(&state.transfer_queue),
            self.event_tx.clone(),
        ))
    }

    fn spawn_transfer<F>(
        &self,
        id: &SessionId,
        direction: TransferDirection,
        local_path: std::path::PathBuf,
        remote_path: String,
        total_bytes: u64,
        perform: F,
    ) -> Result<crate::model::TransferId>
    where
        F: FnOnce(
                Arc<dyn crate::protocol::RemoteProtocol>,
                tokio::sync::mpsc::UnboundedSender<crate::protocol::TransferProgress>,
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
            + Send
            + 'static,
    {
        let proto =
            crate::tokio_handle().block_on(async { self.ensure_sftp_protocol(id).await })?;
        let (session_id, queue, event_tx) = self.session_transfer_context(id)?;
        let local_display = local_path.to_string_lossy().into_owned();
        let tid = queue.enqueue(direction, local_display, remote_path, total_bytes);
        self.emit(SshEvent::TransferChanged(session_id, tid));

        let queue_run = Arc::clone(&queue);
        crate::tokio_handle().spawn(async move {
            queue_run.mark_running(&tid);
            let _ = event_tx.send(SshEvent::TransferChanged(session_id, tid));

            let (progress_tx, mut progress_rx) =
                tokio::sync::mpsc::unbounded_channel::<crate::protocol::TransferProgress>();
            let queue_progress = Arc::clone(&queue_run);
            let event_progress = event_tx.clone();
            let progress_task = tokio::spawn(async move {
                while let Some(p) = progress_rx.recv().await {
                    queue_progress.update_progress(&tid, p.transferred_bytes, p.total_bytes);
                    let _ = event_progress.send(SshEvent::TransferChanged(session_id, tid));
                }
            });

            let result = perform(proto, progress_tx).await;
            progress_task.abort();

            match result {
                Ok(()) => queue_run.mark_completed(&tid),
                Err(error) => queue_run.mark_failed(&tid, &error.to_string()),
            }
            let _ = event_tx.send(SshEvent::TransferChanged(session_id, tid));
        });

        Ok(tid)
    }

    // ========== 传输 ==========

    pub fn upload_file(
        &self,
        id: &SessionId,
        local: &std::path::Path,
        remote: &str,
    ) -> Result<crate::model::TransferId> {
        let total_bytes = std::fs::metadata(local)
            .map(|meta| meta.len())
            .unwrap_or(0);
        let local_path = local.to_path_buf();
        let remote_path = remote.to_string();
        self.spawn_transfer(
            id,
            TransferDirection::Upload,
            local_path.clone(),
            remote_path.clone(),
            total_bytes,
            move |proto, progress_tx| {
                Box::pin(async move { proto.upload_file(&local_path, &remote_path, progress_tx).await })
            },
        )
    }

    pub fn download_file(
        &self,
        id: &SessionId,
        remote: &str,
        local: &std::path::Path,
    ) -> Result<crate::model::TransferId> {
        let local_path = local.to_path_buf();
        let remote_path = remote.to_string();
        self.spawn_transfer(
            id,
            TransferDirection::Download,
            local_path.clone(),
            remote_path.clone(),
            0,
            move |proto, progress_tx| {
                Box::pin(async move {
                    if let Some(parent) = local_path.parent() {
                        std::fs::create_dir_all(parent).ok();
                    }
                    proto
                        .download_file(&remote_path, &local_path, progress_tx)
                        .await
                })
            },
        )
    }

    pub fn session_cwd(&self, id: &SessionId) -> String {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions
            .get(id)
            .map(|s| s.remote_cwd.clone())
            .unwrap_or_default()
    }

    pub fn session_shell_cwd(&self, id: &SessionId) -> Option<String> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.get(id).and_then(|s| s.shell_cwd.clone())
    }

    pub fn set_session_shell_cwd(&self, id: &SessionId, path: impl Into<String>) {
        let path = path.into();
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = sessions.get_mut(id) {
            state.shell_cwd = Some(path);
        }
    }

    pub fn shell_cwd_basis(&self, id: &SessionId) -> String {
        self.session_shell_cwd(id)
            .unwrap_or_else(|| self.session_cwd(id))
    }

    pub fn transfer_snapshots(&self, id: &SessionId) -> Vec<crate::model::TransferTask> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions
            .get(id)
            .map(|s| s.transfer_queue.snapshot())
            .unwrap_or_default()
    }

    // ========== 事件订阅 ==========

    pub fn subscribe(&self) -> broadcast::Receiver<SshEvent> {
        self.event_tx.subscribe()
    }

    /// 应用退出时断开所有连接并清空会话。
    pub fn shutdown_all(&self) {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        let pool = Arc::clone(&self.connection_pool);
        crate::tokio_handle().block_on(async move {
            pool.close_all().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use uuid::Uuid;

    fn init_test_runtime() {
        let _ = crate::tokio_handle();
    }

    fn temp_service() -> SshService {
        init_test_runtime();
        let dir = std::env::temp_dir().join(format!("ssh-svc-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let paths = qingqi_plugin::storage::AppPaths::for_test(dir.clone());
        let database = Arc::new(qingqi_plugin::database::DatabaseService::new(paths));
        let db_path = dir.join("test.db");
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::path(
                "ssh/profiles",
                db_path.clone(),
            ))
            .unwrap();
        let store = Arc::new(crate::store::ProfileStore::new(
            Arc::clone(&database),
            db_path,
        ));
        store.init().unwrap();
        SshService::new(database, store, dir.join("cache"))
    }

    #[test]
    fn test_create_and_list_profiles() {
        let svc = temp_service();
        let draft = ProfileDraft {
            name: "test".into(),
            host: "10.0.0.1".into(),
            port: 22,
            ..Default::default()
        };
        let profile = svc.create_profile(draft).unwrap();
        assert_eq!(profile.name, "test");
        assert_eq!(profile.port, 22);

        let list = svc.list_profiles().unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_snapshot_revision_increments() {
        let svc = temp_service();
        let snap1 = svc.snapshot();
        let draft = ProfileDraft {
            name: "rev-test".into(),
            host: "10.0.0.2".into(),
            port: 22,
            ..Default::default()
        };
        svc.create_profile(draft).unwrap();
        let snap2 = svc.snapshot();
        assert!(snap2.revision > snap1.revision);
    }

    #[test]
    fn test_delete_profile() {
        let svc = temp_service();
        let draft = ProfileDraft {
            name: "del-me".into(),
            host: "10.0.0.3".into(),
            port: 22,
            ..Default::default()
        };
        let p = svc.create_profile(draft).unwrap();
        assert!(svc.delete_profile(p.id).unwrap());
        assert!(svc.get_profile(p.id).unwrap().is_none());
    }
}

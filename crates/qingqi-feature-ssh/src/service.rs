//! 核心服务 — 组装所有子模块

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use tokio::sync::broadcast;
use tracing::{debug, warn};

use crate::connection::ConnectionPool;
use crate::model::{
    Profile, ProfileDraft, RemoteEntry, SessionId, SessionSnapshot, SessionStatus, SessionSummary,
    SshRole, SshSnapshot,
};
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
    transfer_queue: Arc<TransferQueue>,
}

// ============ SshService ============

pub struct SshService {
    profile_store: Arc<ProfileStore>,
    #[allow(dead_code)]
    cache_dir: PathBuf, // 终端历史/临时文件目录
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

        Self {
            profile_store,
            cache_dir,
            connection_pool: Arc::new(ConnectionPool::new()),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            revision: AtomicU64::new(0),
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
                        Ok(rx) => {
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
                                rx,
                                Some(Box::new(move || {
                                    Self::notify_async(
                                        SshEvent::SessionDataChanged(sid_term),
                                        &tx_term,
                                    );
                                })),
                            );
                            Some(engine)
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

    pub fn close_session(&self, id: &SessionId) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = sessions.remove(id) {
            let pool = Arc::clone(&self.connection_pool);
            let profile_id = state.profile_id;
            crate::tokio_handle().spawn(async move {
                pool.disconnect_all(profile_id).await;
            });
        }
        drop(sessions);
        self.emit(SshEvent::SessionClosed(*id));
        Ok(())
    }

    pub fn session_summaries(&self) -> Vec<SessionSummary> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.values().map(|s| s.summary.clone()).collect()
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

    pub fn send_terminal_input(&self, id: &SessionId, data: &[u8]) -> Result<()> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let state = sessions
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Session 不存在"))?;
        if let Some(proto) = &state.terminal_protocol {
            let proto = Arc::clone(proto);
            let data = data.to_vec();
            crate::tokio_handle().spawn(async move {
                let _ = proto.send_terminal_input(&data).await;
            });
        }
        Ok(())
    }

    // ========== 文件操作 ==========

    pub fn list_directory(&self, id: &SessionId, path: &str) -> Result<Vec<RemoteEntry>> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let state = sessions
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Session 不存在"))?;
        let proto = state
            .sftp_protocol
            .clone()
            .ok_or_else(|| anyhow::anyhow!("SFTP 未连接"))?;
        drop(sessions);

        let entries = crate::tokio_handle().block_on(async { proto.list_directory(path).await })?;
        let resolved = proto.last_list_path().unwrap_or_else(|| path.to_string());

        // 更新缓存
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
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let proto = sessions
            .get(id)
            .and_then(|s| s.sftp_protocol.clone())
            .ok_or_else(|| anyhow::anyhow!("SFTP 未连接"))?;
        drop(sessions);
        crate::tokio_handle().block_on(async { proto.create_directory(path).await })
    }

    pub fn session_cwd(&self, id: &SessionId) -> String {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions
            .get(id)
            .map(|s| s.remote_cwd.clone())
            .unwrap_or_default()
    }

    // ========== 传输 ==========

    pub fn upload_file(
        &self,
        id: &SessionId,
        local: &std::path::Path,
        remote: &str,
    ) -> Result<crate::model::TransferId> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let state = sessions
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Session 不存在"))?;
        let proto = state
            .sftp_protocol
            .clone()
            .ok_or_else(|| anyhow::anyhow!("SFTP 未连接"))?;
        drop(sessions);

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let tid = crate::model::TransferId::new();
        let proto_clone = Arc::clone(&proto);
        let local_path = local.to_path_buf();
        let remote_path = remote.to_string();

        crate::tokio_handle().spawn(async move {
            match proto_clone.upload_file(&local_path, &remote_path, tx).await {
                Ok(_) => { /* 传输完成 */ }
                Err(_) => { /* 传输失败 */ }
            }
        });

        Ok(tid)
    }

    pub fn download_file(
        &self,
        id: &SessionId,
        remote: &str,
        local: &std::path::Path,
    ) -> Result<crate::model::TransferId> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let state = sessions
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Session 不存在"))?;
        let proto = state
            .sftp_protocol
            .clone()
            .ok_or_else(|| anyhow::anyhow!("SFTP 未连接"))?;
        drop(sessions);

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let tid = crate::model::TransferId::new();
        let proto_clone = Arc::clone(&proto);
        let local_path = local.to_path_buf();
        let remote_path = remote.to_string();

        crate::tokio_handle().spawn(async move {
            match proto_clone
                .download_file(&remote_path, &local_path, tx)
                .await
            {
                Ok(_) => {}
                Err(_) => {}
            }
        });

        Ok(tid)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use uuid::Uuid;

    fn temp_service() -> SshService {
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

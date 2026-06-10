//! 核心服务 — 组装所有子模块

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use tokio::sync::broadcast;

use crate::connection::{default_registry, ConnectionPool};
use crate::model::{
    Profile, ProfileDraft, RemoteEntry, SessionId, SessionSnapshot, SessionStatus,
    SessionSummary, SshSnapshot,
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
    protocol: Option<Arc<dyn crate::protocol::RemoteProtocol>>,
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
        let registry = default_registry();
        let (event_tx, _) = broadcast::channel(256);

        Self {
            profile_store,
            cache_dir,
            connection_pool: Arc::new(ConnectionPool::new(registry)),
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
        let _ = self.event_tx.send(event);
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
        let profile = self
            .get_profile(profile_id)?
            .ok_or_else(|| anyhow::anyhow!("Profile {profile_id} 不存在"))?;

        let session_id = SessionId::new();
        let terminal_kind = profile.protocol.supports_terminal();

        let summary = SessionSummary {
            session_id,
            profile_id,
            title: format!("{}:{}", profile.host, profile.port),
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
                protocol: None,
                terminal: None,
                entries: Vec::new(),
                remote_cwd: profile.paths.remote_root.clone(),
                transfer_queue: Arc::new(TransferQueue::new(session_id)),
            },
        );
        drop(sessions);

        self.emit(SshEvent::SessionOpened(session_id));

        // 异步连接
        let pool = Arc::clone(&self.connection_pool);
        let sessions = Arc::clone(&self.sessions);
        let tx = self.event_tx.clone();
        let sid = session_id;
        let p = profile;
        tokio::spawn(async move {
            match pool.get_or_connect(&p).await {
                Ok(proto) => {
                    // 打开终端
                    let term = match proto.open_terminal().await {
                        Ok(rx) => {
                            let engine = Arc::new(TerminalEngine::new(p.protocol.supports_terminal()));
                            engine.set_status(&format!("{}@{}:{}", p.name, p.host, p.port));
                            TerminalEngine::start_processing(engine.clone(), rx);
                            Some(engine)
                        }
                        Err(_) => None,
                    };

                    // 更新 session 状态
                    let mut sessions_guard = sessions.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(state) = sessions_guard.get_mut(&sid) {
                        state.protocol = Some(proto);
                        state.terminal = term;
                        state.summary.status = SessionStatus::Connected;
                        state.summary.message = "已连接".into();
                    }
                    drop(sessions_guard);
                    let _ = tx.send(SshEvent::SessionConnected(sid));
                }
                Err(e) => {
                    let mut sessions_guard = sessions.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(state) = sessions_guard.get_mut(&sid) {
                        state.summary.status = SessionStatus::Failed;
                        state.summary.message = format!("连接失败: {e}");
                    }
                    drop(sessions_guard);
                    let _ = tx.send(SshEvent::SessionDataChanged(sid));
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
            tokio::spawn(async move {
                pool.disconnect(profile_id).await;
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
        if let Some(proto) = &state.protocol {
            let proto = Arc::clone(proto);
            let data = data.to_vec();
            tokio::spawn(async move {
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
            .protocol
            .clone()
            .ok_or_else(|| anyhow::anyhow!("未连接"))?;
        // 需要在锁外调用 async，先克隆 protocol
        drop(sessions);

        let entries = tokio::runtime::Handle::current()
            .block_on(async { proto.list_directory(path).await })?;

        // 更新缓存
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = sessions.get_mut(id) {
            state.entries = entries.clone();
            state.remote_cwd = path.to_string();
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
            .protocol
            .clone()
            .ok_or_else(|| anyhow::anyhow!("未连接"))?;
        drop(sessions);

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let tid = crate::model::TransferId::new();
        let proto_clone = Arc::clone(&proto);
        let local_path = local.to_path_buf();
        let remote_path = remote.to_string();

        tokio::spawn(async move {
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
            .protocol
            .clone()
            .ok_or_else(|| anyhow::anyhow!("未连接"))?;
        drop(sessions);

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let tid = crate::model::TransferId::new();
        let proto_clone = Arc::clone(&proto);
        let local_path = local.to_path_buf();
        let remote_path = remote.to_string();

        tokio::spawn(async move {
            match proto_clone.download_file(&remote_path, &local_path, tx).await {
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
            .register_database(qingqi_plugin::database::DatabaseSpec::path("ssh/profiles", db_path.clone()))
            .unwrap();
        let store = Arc::new(crate::store::ProfileStore::new(Arc::clone(&database), db_path));
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

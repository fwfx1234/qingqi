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
    sessions: Mutex<HashMap<SessionId, SessionState>>,
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
            sessions: Mutex::new(HashMap::new()),
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
            title: format!("{}@{}", profile.name, profile.host),
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
        let tx = self.event_tx.clone();
        let sid = session_id;
        let p = profile;
        tokio::spawn(async move {
            match pool.get_or_connect(&p).await {
                Ok(_proto) => {
                    let _ = tx.send(SshEvent::SessionConnected(sid));
                }
                Err(_e) => {
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

    // ========== 文件操作 ==========

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

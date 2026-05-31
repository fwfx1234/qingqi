use std::{collections::HashMap, sync::Mutex};

use super::backend::RemoteBackend;

/// Thread-safe registry of active remote sessions, keyed by profile id.
pub struct RemoteConnectionPool {
    sessions: Mutex<HashMap<i64, Box<dyn RemoteBackend>>>,
}

impl RemoteConnectionPool {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn insert(&self, id: i64, backend: Box<dyn RemoteBackend>) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.insert(id, backend);
        }
    }

    pub fn remove(&self, id: i64) -> Option<Box<dyn RemoteBackend>> {
        self.sessions.lock().ok()?.remove(&id)
    }

    pub fn with_backend<F, R>(&self, id: i64, f: F) -> Option<R>
    where
        F: FnOnce(&dyn RemoteBackend) -> R,
    {
        let sessions = self.sessions.lock().ok()?;
        let backend = sessions.get(&id)?;
        Some(f(backend.as_ref()))
    }

    pub fn close_all(&self) {
        if let Ok(mut sessions) = self.sessions.lock() {
            for (_, mut backend) in sessions.drain() {
                backend.close();
            }
        }
    }

    pub fn has(&self, id: i64) -> bool {
        self.sessions
            .lock()
            .map(|s| s.contains_key(&id))
            .unwrap_or(false)
    }
}

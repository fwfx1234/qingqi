use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Condvar, Mutex};

use crate::model::{SessionId, TransferId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TransferStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferSnapshot {
    pub id: TransferId,
    pub session_id: SessionId,
    pub direction: TransferDirection,
    pub local_path: String,
    pub remote_path: String,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub status: TransferStatus,
    pub message: String,
}

impl TransferSnapshot {
    pub fn progress_percent(&self) -> Option<u8> {
        if self.total_bytes == 0 {
            return None;
        }
        Some(((self.transferred_bytes as f64 / self.total_bytes as f64) * 100.0).clamp(0.0, 100.0) as u8)
    }
}

#[derive(Default)]
struct TransferState {
    items: HashMap<TransferId, TransferSnapshot>,
    queue_order: VecDeque<TransferId>,
    running_by_session: HashMap<SessionId, usize>,
    limits_by_session: HashMap<SessionId, usize>,
    cancelled: HashSet<TransferId>,
}

pub struct TransferQueue {
    default_concurrency_per_session: usize,
    state: Mutex<TransferState>,
    slot_available: Condvar,
}

impl TransferQueue {
    pub fn new(concurrency_per_session: usize) -> Self {
        Self {
            default_concurrency_per_session: concurrency_per_session.max(1),
            state: Mutex::new(TransferState::default()),
            slot_available: Condvar::new(),
        }
    }

    pub fn set_session_limit(&self, session_id: SessionId, concurrency: usize) {
        if let Ok(mut state) = self.state.lock() {
            state
                .limits_by_session
                .insert(session_id, concurrency.max(1));
        }
    }

    pub fn remove_session_limit(&self, session_id: &SessionId) {
        if let Ok(mut state) = self.state.lock() {
            state.limits_by_session.remove(session_id);
            state.running_by_session.remove(session_id);
        }
    }

    pub fn enqueue(
        &self,
        session_id: SessionId,
        direction: TransferDirection,
        local_path: String,
        remote_path: String,
        total_bytes: u64,
    ) -> TransferId {
        let id = TransferId::new();
        let mut state = self.state.lock().expect("transfer queue lock");
        state.items.insert(
            id.clone(),
            TransferSnapshot {
                id: id.clone(),
                session_id,
                direction,
                local_path,
                remote_path,
                transferred_bytes: 0,
                total_bytes,
                status: TransferStatus::Queued,
                message: String::from("排队中"),
            },
        );
        state.queue_order.push_back(id.clone());
        id
    }

    pub fn start_next_for_session(&self, session_id: &SessionId) -> Option<TransferId> {
        let mut state = self.state.lock().ok()?;
        let running = state
            .running_by_session
            .get(session_id)
            .copied()
            .unwrap_or(0);
        let limit = state
            .limits_by_session
            .get(session_id)
            .copied()
            .unwrap_or(self.default_concurrency_per_session);
        if running >= limit {
            return None;
        }
        let next_index = state.queue_order.iter().position(|id| {
            state.items.get(id).is_some_and(|item| {
                item.session_id == *session_id && item.status == TransferStatus::Queued
            })
        })?;
        let next_id = state.queue_order.remove(next_index)?;
        if let Some(item) = state.items.get_mut(&next_id) {
            item.status = TransferStatus::Running;
            item.message = String::from("传输中");
        }
        *state
            .running_by_session
            .entry(session_id.clone())
            .or_default() += 1;
        Some(next_id)
    }

    pub fn try_start(&self, id: &TransferId) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        matches!(
            try_start_locked(&mut state, id, self.default_concurrency_per_session),
            StartOutcome::Started
        )
    }

    /// 阻塞等待该传输拿到一个并发槽位（事件驱动，不轮询）。
    /// 返回 true 表示已置为 Running；false 表示已被取消/不存在。
    pub fn wait_and_start(&self, id: &TransferId) -> bool {
        let mut state = self.state.lock().expect("transfer queue lock");
        loop {
            match try_start_locked(&mut state, id, self.default_concurrency_per_session) {
                StartOutcome::Started => return true,
                StartOutcome::Cancelled => return false,
                StartOutcome::WouldBlock => {
                    state = self
                        .slot_available
                        .wait(state)
                        .expect("transfer queue condvar");
                }
            }
        }
    }

    pub fn mark_progress(&self, id: &TransferId, transferred: u64, total: Option<u64>) {
        if let Ok(mut state) = self.state.lock()
            && let Some(item) = state.items.get_mut(id)
        {
            item.transferred_bytes = transferred;
            if let Some(total) = total {
                item.total_bytes = total;
            }
        }
    }

    pub fn finish(&self, id: &TransferId, success: bool, message: String) {
        if let Ok(mut state) = self.state.lock() {
            let was_cancelled = state.cancelled.remove(id);
            let session_id = state.items.get(id).map(|item| item.session_id.clone());

            if let Some(item) = state.items.get_mut(id) {
                if was_cancelled {
                    item.status = TransferStatus::Cancelled;
                } else {
                    item.status = if success {
                        TransferStatus::Completed
                    } else {
                        TransferStatus::Failed
                    };
                }
                item.message = message;
            }

            if let Some(session_id) = session_id {
                let running = state.running_by_session.entry(session_id).or_default();
                *running = running.saturating_sub(1);
            }
            self.slot_available.notify_all();
        }
    }

    pub fn cancel(&self, id: &TransferId) {
        if let Ok(mut state) = self.state.lock() {
            state.cancelled.insert(id.clone());
            state.queue_order.retain(|queued_id| queued_id != id);
            if let Some(item) = state.items.get_mut(id)
                && !item.status.is_terminal()
            {
                item.status = TransferStatus::Cancelled;
                item.message = String::from("已取消");
            }
            self.slot_available.notify_all();
        }
    }

    pub fn snapshot(&self, id: &TransferId) -> Option<TransferSnapshot> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.items.get(id).cloned())
    }

    pub fn snapshots_for_session(&self, session_id: &SessionId) -> Vec<TransferSnapshot> {
        self.state
            .lock()
            .map(|state| {
                state
                    .items
                    .values()
                    .filter(|item| &item.session_id == session_id)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn all_snapshots(&self) -> Vec<TransferSnapshot> {
        self.state
            .lock()
            .map(|state| state.items.values().cloned().collect())
            .unwrap_or_default()
    }
}

enum StartOutcome {
    Started,
    Cancelled,
    WouldBlock,
}

/// 在已持有锁的前提下尝试启动一个传输。抽出供 `try_start`（非阻塞）与
/// `wait_and_start`（阻塞等槽位）复用，避免重复加锁。
fn try_start_locked(
    state: &mut TransferState,
    id: &TransferId,
    default_limit: usize,
) -> StartOutcome {
    let Some(item) = state.items.get(id).cloned() else {
        return StartOutcome::Cancelled;
    };
    if item.status != TransferStatus::Queued {
        return if item.status == TransferStatus::Running {
            StartOutcome::Started
        } else {
            StartOutcome::Cancelled
        };
    }
    let running = state
        .running_by_session
        .get(&item.session_id)
        .copied()
        .unwrap_or(0);
    let limit = state
        .limits_by_session
        .get(&item.session_id)
        .copied()
        .unwrap_or(default_limit);
    if running >= limit {
        return StartOutcome::WouldBlock;
    }
    if let Some(index) = state
        .queue_order
        .iter()
        .position(|queued_id| queued_id == id)
    {
        state.queue_order.remove(index);
    }
    if let Some(existing) = state.items.get_mut(id) {
        existing.status = TransferStatus::Running;
        existing.message = String::from("传输中");
    }
    *state.running_by_session.entry(item.session_id).or_default() += 1;
    StartOutcome::Started
}

#[cfg(test)]
mod tests {
    use crate::model::SessionId;

    use super::{TransferDirection, TransferQueue, TransferStatus};

    #[test]
    fn respects_per_session_concurrency() {
        let queue = TransferQueue::new(1);
        let session = SessionId::new();
        queue.enqueue(
            session.clone(),
            TransferDirection::Upload,
            "a".into(),
            "b".into(),
            10,
        );
        queue.enqueue(
            session.clone(),
            TransferDirection::Upload,
            "c".into(),
            "d".into(),
            10,
        );
        assert!(queue.start_next_for_session(&session).is_some());
        assert!(queue.start_next_for_session(&session).is_none());
    }

    #[test]
    fn cancel_marks_transfer_terminal() {
        let queue = TransferQueue::new(1);
        let session = SessionId::new();
        let id = queue.enqueue(
            session.clone(),
            TransferDirection::Download,
            "a".into(),
            "b".into(),
            10,
        );
        queue.cancel(&id);
        let item = queue
            .snapshots_for_session(&session)
            .pop()
            .expect("snapshot");
        assert_eq!(item.status, TransferStatus::Cancelled);
    }

    #[test]
    fn respects_session_specific_limit() {
        let queue = TransferQueue::new(1);
        let session = SessionId::new();
        queue.set_session_limit(session.clone(), 2);
        queue.enqueue(
            session.clone(),
            TransferDirection::Upload,
            "a".into(),
            "b".into(),
            10,
        );
        queue.enqueue(
            session.clone(),
            TransferDirection::Upload,
            "c".into(),
            "d".into(),
            10,
        );
        queue.enqueue(
            session.clone(),
            TransferDirection::Upload,
            "e".into(),
            "f".into(),
            10,
        );
        assert!(queue.start_next_for_session(&session).is_some());
        assert!(queue.start_next_for_session(&session).is_some());
        assert!(queue.start_next_for_session(&session).is_none());
    }

    #[test]
    fn preserves_queue_order_within_session() {
        let queue = TransferQueue::new(1);
        let session = SessionId::new();
        let first = queue.enqueue(
            session.clone(),
            TransferDirection::Upload,
            "a".into(),
            "1".into(),
            10,
        );
        let second = queue.enqueue(
            session.clone(),
            TransferDirection::Upload,
            "b".into(),
            "2".into(),
            10,
        );
        assert_eq!(queue.start_next_for_session(&session), Some(first.clone()));
        queue.finish(&first, true, String::from("done"));
        assert_eq!(queue.start_next_for_session(&session), Some(second));
    }
}

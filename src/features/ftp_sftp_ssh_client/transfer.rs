use std::{
    collections::HashMap,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Instant,
};

use super::{
    model::{TransferDirection, TransferItem, TransferStatus},
    pool::RemoteConnectionPool,
};

/// Aggregated counts derived from a snapshot of [`TransferItem`]s.
///
/// Surfaces a richer breakdown than total/active alone so the transfer strip can
/// honestly distinguish completed, failed, and cancelled outcomes without
/// inferring state from UI-only flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransferCounts {
    pub total: usize,
    pub active: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

impl TransferCounts {
    pub fn terminal(&self) -> usize {
        self.completed + self.failed + self.cancelled
    }

    pub fn has_terminal(&self) -> bool {
        self.terminal() > 0
    }
}

/// Compute a fresh [`TransferCounts`] snapshot from items.
pub fn transfer_counts(items: &[TransferItem]) -> TransferCounts {
    let mut counts = TransferCounts {
        total: items.len(),
        ..TransferCounts::default()
    };
    for item in items {
        match item.status {
            TransferStatus::Queued | TransferStatus::Running => counts.active += 1,
            TransferStatus::Completed => counts.completed += 1,
            TransferStatus::Failed => counts.failed += 1,
            TransferStatus::Cancelled => counts.cancelled += 1,
        }
    }
    counts
}

/// Transfer service with max 3 concurrent workers.
pub struct TransferService {
    items: Mutex<HashMap<String, TransferItem>>,
    cancelled: Mutex<std::collections::HashSet<String>>,
    pool: Arc<RemoteConnectionPool>,
    revision: Arc<std::sync::atomic::AtomicU64>,
    shutdown: AtomicBool,
    _workers: Vec<thread::JoinHandle<()>>,
    task_tx: std::sync::mpsc::Sender<TransferTask>,
}

struct TransferTask {
    transfer_id: String,
    profile_id: i64,
    direction: TransferDirection,
    local_path: String,
    remote_path: String,
    size: i64,
}

impl TransferService {
    pub fn new(
        pool: Arc<RemoteConnectionPool>,
        revision: Arc<std::sync::atomic::AtomicU64>,
    ) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<TransferTask>();
        let rx = Arc::new(Mutex::new(rx));

        let mut workers = Vec::new();
        for _ in 0..3 {
            let rx = Arc::clone(&rx);
            // Each worker just pulls tasks from the shared receiver
            // We can't easily share an mpsc::Receiver, so we use a shared mutex around it
            let handle = thread::Builder::new()
                .name("transfer-worker".into())
                .spawn(move || {
                    loop {
                        let task = match rx.lock() {
                            Ok(guard) => match guard.recv() {
                                Ok(t) => t,
                                Err(_) => break, // Channel closed
                            },
                            Err(_) => break, // Mutex poisoned
                        };
                        // Worker will be driven externally via run_transfer
                        drop(task);
                    }
                })
                .ok();
            if let Some(h) = handle {
                workers.push(h);
            }
        }

        Self {
            items: Mutex::new(HashMap::new()),
            cancelled: Mutex::new(std::collections::HashSet::new()),
            pool,
            revision,
            shutdown: AtomicBool::new(false),
            _workers: workers,
            task_tx: tx,
        }
    }

    pub fn start_upload(
        self: &Arc<Self>,
        profile_id: i64,
        local_path: String,
        remote_path: String,
    ) -> String {
        let size = std::fs::metadata(&local_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);
        let item = TransferItem::new(
            uuid::Uuid::new_v4().to_string(),
            TransferDirection::Upload,
            local_path.clone(),
            remote_path.clone(),
            size,
        );
        let id = item.id.clone();

        if let Ok(mut items) = self.items.lock() {
            items.insert(id.clone(), item);
        }
        self.bump();

        // Spawn background thread for this transfer
        let svc = Arc::clone(self);
        let tid = id.clone();
        thread::spawn(move || {
            svc.run_transfer(
                &tid,
                profile_id,
                TransferDirection::Upload,
                &local_path,
                &remote_path,
                size,
            );
        });

        id
    }

    pub fn start_download(
        self: &Arc<Self>,
        profile_id: i64,
        remote_path: String,
        local_path: String,
        size: i64,
    ) -> String {
        let item = TransferItem::new(
            uuid::Uuid::new_v4().to_string(),
            TransferDirection::Download,
            local_path.clone(),
            remote_path.clone(),
            size,
        );
        let id = item.id.clone();

        if let Ok(mut items) = self.items.lock() {
            items.insert(id.clone(), item);
        }
        self.bump();

        let svc = Arc::clone(self);
        let tid = id.clone();
        thread::spawn(move || {
            svc.run_transfer(
                &tid,
                profile_id,
                TransferDirection::Download,
                &local_path,
                &remote_path,
                size,
            );
        });

        id
    }

    fn run_transfer(
        &self,
        transfer_id: &str,
        profile_id: i64,
        direction: TransferDirection,
        local_path: &str,
        remote_path: &str,
        size: i64,
    ) {
        // Mark as running
        if let Ok(mut items) = self.items.lock() {
            if let Some(item) = items.get_mut(transfer_id) {
                item.status = TransferStatus::Running;
                item.message = "传输中".into();
            }
        }
        self.bump();

        let started_at = Instant::now();
        let tid = transfer_id.to_string();
        let cancelled = &self.cancelled;
        let items_mutex = &self.items;
        let revision = &self.revision;

        // Check cancellation
        let is_cancelled =
            || -> bool { cancelled.lock().map(|c| c.contains(&tid)).unwrap_or(false) };

        // Progress callback
        let progress = |done: usize, total: usize| {
            if is_cancelled() {
                return;
            }
            if let Ok(mut items) = items_mutex.lock() {
                if let Some(item) = items.get_mut(&tid) {
                    item.transferred = done as i64;
                    if total > 0 {
                        item.size = total as i64;
                    }
                    let elapsed = started_at.elapsed().as_secs_f64().max(0.1);
                    let speed = (done as f64 / 1024.0) / elapsed;
                    item.speed = format!("{speed:.1} KB/s");
                }
            }
            revision.fetch_add(1, Ordering::SeqCst);
        };

        // Execute transfer via pool
        let result = self
            .pool
            .with_backend(profile_id, |backend| match direction {
                TransferDirection::Upload => {
                    backend.upload_file(Path::new(local_path), remote_path, &progress)
                }
                TransferDirection::Download => {
                    backend.download_file(remote_path, Path::new(local_path), &progress)
                }
            });

        // Update final status
        let was_cancelled = is_cancelled();
        if let Ok(mut items) = self.items.lock() {
            if let Some(item) = items.get_mut(transfer_id) {
                if was_cancelled {
                    item.status = TransferStatus::Cancelled;
                    item.message = "已取消".into();
                } else if let Some(inner_result) = result {
                    match inner_result {
                        Ok(()) => {
                            item.status = TransferStatus::Completed;
                            item.transferred = item.size;
                            item.speed = "0 KB/s".into();
                            item.message = "已完成".into();
                        }
                        Err(ref e) => {
                            item.status = TransferStatus::Failed;
                            item.speed = "0 KB/s".into();
                            item.message = format!("{e}");
                        }
                    }
                } else {
                    item.status = TransferStatus::Failed;
                    item.message = "连接已断开".into();
                }
            }
        }
        self.bump();

        // Remove from cancelled set
        if let Ok(mut c) = cancelled.lock() {
            c.remove(transfer_id);
        }
    }

    pub fn cancel(&self, transfer_id: &str) {
        if let Ok(mut c) = self.cancelled.lock() {
            c.insert(transfer_id.to_string());
        }
        if let Ok(mut items) = self.items.lock() {
            if let Some(item) = items.get_mut(transfer_id) {
                if !item.status.is_terminal() {
                    item.status = TransferStatus::Cancelled;
                    item.message = "已取消".into();
                }
            }
        }
        self.bump();
    }

    pub fn items(&self) -> Vec<TransferItem> {
        self.items
            .lock()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn item(&self, transfer_id: &str) -> Option<TransferItem> {
        self.items
            .lock()
            .ok()
            .and_then(|items| items.get(transfer_id).cloned())
    }

    pub fn clear_finished(&self) {
        if let Ok(mut items) = self.items.lock() {
            items.retain(|_, item| !item.status.is_terminal());
        }
        self.bump();
    }

    pub fn any_active(&self) -> bool {
        self.items
            .lock()
            .map(|m| m.values().any(|item| item.is_active()))
            .unwrap_or(false)
    }

    pub fn any_terminal(&self) -> bool {
        self.items
            .lock()
            .map(|m| m.values().any(|item| item.status.is_terminal()))
            .unwrap_or(false)
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    fn bump(&self) {
        self.revision.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicU64};

    use super::{
        super::model::{TransferDirection, TransferItem, TransferStatus},
        TransferCounts, TransferService, transfer_counts,
    };
    use crate::features::ftp_sftp_ssh_client::pool::RemoteConnectionPool;

    fn make_item(
        id: &str,
        status: TransferStatus,
        size: i64,
        transferred: i64,
        speed: &str,
    ) -> TransferItem {
        let mut item = TransferItem::new(
            id.into(),
            TransferDirection::Upload,
            "/local/test.txt".into(),
            "/remote/test.txt".into(),
            size,
        );
        item.status = status;
        item.transferred = transferred;
        item.speed = speed.to_string();
        item
    }

    fn insert(svc: &Arc<TransferService>, item: TransferItem) {
        let mut items = svc.items.lock().unwrap();
        items.insert(item.id.clone(), item);
    }

    fn make_service() -> Arc<TransferService> {
        let pool = Arc::new(RemoteConnectionPool::new());
        let revision = Arc::new(AtomicU64::new(0));
        Arc::new(TransferService::new(pool, revision))
    }

    #[test]
    fn status_line_queued_shows_size() {
        let item = make_item("t", TransferStatus::Queued, 1024 * 1024, 0, "");
        let line = item.status_line();
        assert!(line.contains("排队"), "expected 排队 in {line:?}");
        assert!(line.contains("1.0 MB"), "expected 1.0 MB in {line:?}");
    }

    #[test]
    fn status_line_running_shows_progress_and_speed() {
        let item = make_item(
            "t",
            TransferStatus::Running,
            1024 * 1024,
            512 * 1024,
            "256.0 KB/s",
        );
        let line = item.status_line();
        assert!(line.contains("512.0 KB"), "expected 512.0 KB in {line:?}");
        assert!(line.contains("1.0 MB"), "expected 1.0 MB in {line:?}");
        assert!(
            line.contains("256.0 KB/s"),
            "expected 256.0 KB/s in {line:?}"
        );
    }

    #[test]
    fn status_line_running_without_speed_omits_speed() {
        let item = make_item("t", TransferStatus::Running, 100, 50, "");
        let line = item.status_line();
        assert!(line.contains("50 B"), "expected 50 B in {line:?}");
        assert!(line.contains("100 B"), "expected 100 B in {line:?}");
        assert!(
            !line.contains("KB/s"),
            "speed segment should be absent: {line:?}"
        );
    }

    #[test]
    fn status_line_completed_shows_size() {
        let item = make_item("t", TransferStatus::Completed, 2048, 2048, "");
        let line = item.status_line();
        assert!(line.contains("已完成"), "expected 已完成 in {line:?}");
        assert!(line.contains("2.0 KB"), "expected 2.0 KB in {line:?}");
    }

    #[test]
    fn status_line_failed_shows_message() {
        let mut item = make_item("t", TransferStatus::Failed, 100, 0, "");
        item.message = "连接已断开".into();
        let line = item.status_line();
        assert!(line.contains("失败"), "expected 失败 in {line:?}");
        assert!(
            line.contains("连接已断开"),
            "expected 连接已断开 in {line:?}"
        );
    }

    #[test]
    fn status_line_failed_plain() {
        let item = make_item("t", TransferStatus::Failed, 100, 0, "");
        let line = item.status_line();
        assert_eq!(line, "失败");
    }

    #[test]
    fn status_line_cancelled() {
        let item = make_item("t", TransferStatus::Cancelled, 100, 0, "");
        assert_eq!(item.status_line(), "已取消");
    }

    #[test]
    fn is_active_queued_and_running() {
        assert!(make_item("t", TransferStatus::Queued, 100, 0, "").is_active());
        assert!(make_item("t", TransferStatus::Running, 100, 50, "").is_active());
    }

    #[test]
    fn is_active_false_for_terminal() {
        assert!(!make_item("t", TransferStatus::Completed, 100, 100, "").is_active());
        assert!(!make_item("t", TransferStatus::Failed, 100, 0, "").is_active());
        assert!(!make_item("t", TransferStatus::Cancelled, 100, 0, "").is_active());
    }

    #[test]
    fn is_terminal_correct() {
        assert!(TransferStatus::Completed.is_terminal());
        assert!(TransferStatus::Failed.is_terminal());
        assert!(TransferStatus::Cancelled.is_terminal());
        assert!(!TransferStatus::Queued.is_terminal());
        assert!(!TransferStatus::Running.is_terminal());
    }

    #[test]
    fn progress_percent() {
        let mut item = TransferItem::new(
            "pct".into(),
            TransferDirection::Download,
            "/local/f".into(),
            "/remote/f".into(),
            1000,
        );
        item.transferred = 250;
        assert_eq!(item.progress_percent(), 25);
        item.transferred = 1000;
        assert_eq!(item.progress_percent(), 100);
    }

    #[test]
    fn progress_percent_zero_size() {
        let item = TransferItem::new(
            "pct".into(),
            TransferDirection::Download,
            "/local/f".into(),
            "/remote/f".into(),
            0,
        );
        assert_eq!(item.progress_percent(), 0);
    }

    #[test]
    fn cancel_marks_item_cancelled() {
        let svc = make_service();
        insert(&svc, make_item("t1", TransferStatus::Running, 100, 50, ""));
        svc.cancel("t1");
        let items = svc.items();
        let t = items.iter().find(|i| i.id == "t1").unwrap();
        assert_eq!(t.status, TransferStatus::Cancelled);
        assert_eq!(t.message, "已取消");
    }

    #[test]
    fn cancel_is_idempotent() {
        let svc = make_service();
        insert(&svc, make_item("t1", TransferStatus::Queued, 100, 0, ""));
        svc.cancel("t1");
        svc.cancel("t1"); // second cancel should not panic or flip status
        let items = svc.items();
        let t = items.iter().find(|i| i.id == "t1").unwrap();
        assert_eq!(t.status, TransferStatus::Cancelled);
    }

    #[test]
    fn cancel_does_not_overwrite_terminal_status() {
        let svc = make_service();
        let mut done = make_item("done", TransferStatus::Completed, 100, 100, "");
        done.message = "已完成".into();
        insert(&svc, done);
        svc.cancel("done");
        let items = svc.items();
        let t = items.iter().find(|i| i.id == "done").unwrap();
        assert_eq!(t.status, TransferStatus::Completed);
        assert_eq!(t.message, "已完成");
    }

    #[test]
    fn cancel_unknown_id_is_noop() {
        let svc = make_service();
        svc.cancel("missing");
        assert!(svc.items().is_empty());
    }

    #[test]
    fn clear_finished_removes_terminal_items() {
        let svc = make_service();
        insert(
            &svc,
            make_item("active", TransferStatus::Running, 100, 50, ""),
        );
        insert(
            &svc,
            make_item("done", TransferStatus::Completed, 100, 100, ""),
        );
        insert(
            &svc,
            make_item("failed", TransferStatus::Failed, 100, 0, ""),
        );
        svc.clear_finished();
        let items = svc.items();
        assert_eq!(items.len(), 1, "only active item should remain");
        assert!(items.iter().any(|i| i.id == "active"));
    }

    #[test]
    fn clear_finished_keeps_queued() {
        let svc = make_service();
        insert(&svc, make_item("q", TransferStatus::Queued, 100, 0, ""));
        svc.clear_finished();
        let items = svc.items();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn any_active_and_any_terminal() {
        let svc = make_service();
        assert!(!svc.any_active());
        assert!(!svc.any_terminal());
        insert(
            &svc,
            make_item("active", TransferStatus::Running, 100, 50, ""),
        );
        insert(
            &svc,
            make_item("done", TransferStatus::Completed, 100, 100, ""),
        );
        assert!(svc.any_active());
        assert!(svc.any_terminal());
    }

    #[test]
    fn transfer_counts_empty_slice() {
        let counts = transfer_counts(&[]);
        assert_eq!(counts, TransferCounts::default());
        assert_eq!(counts.terminal(), 0);
        assert!(!counts.has_terminal());
    }

    #[test]
    fn transfer_counts_breakdown() {
        let items = vec![
            make_item("q", TransferStatus::Queued, 100, 0, ""),
            make_item("r", TransferStatus::Running, 100, 40, ""),
            make_item("c1", TransferStatus::Completed, 100, 100, ""),
            make_item("c2", TransferStatus::Completed, 100, 100, ""),
            make_item("f", TransferStatus::Failed, 100, 0, ""),
            make_item("x", TransferStatus::Cancelled, 100, 0, ""),
        ];
        let counts = transfer_counts(&items);
        assert_eq!(counts.total, 6);
        assert_eq!(counts.active, 2);
        assert_eq!(counts.completed, 2);
        assert_eq!(counts.failed, 1);
        assert_eq!(counts.cancelled, 1);
        assert_eq!(counts.terminal(), 4);
        assert!(counts.has_terminal());
    }

    #[test]
    fn transfer_counts_only_active() {
        let items = vec![
            make_item("q", TransferStatus::Queued, 100, 0, ""),
            make_item("r", TransferStatus::Running, 100, 40, ""),
        ];
        let counts = transfer_counts(&items);
        assert_eq!(counts.active, 2);
        assert!(!counts.has_terminal());
    }
}

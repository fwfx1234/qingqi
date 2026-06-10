//! 传输队列

use std::sync::Mutex;

use crate::model::{SessionId, TransferDirection, TransferId, TransferStatus, TransferTask};

const MAX_CONCURRENT: usize = 3;

pub struct TransferQueue {
    session_id: SessionId,
    tasks: Mutex<Vec<TransferTask>>,
}

impl TransferQueue {
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            tasks: Mutex::new(Vec::new()),
        }
    }

    pub fn enqueue(
        &self,
        direction: TransferDirection,
        local_path: String,
        remote_path: String,
        total_bytes: u64,
    ) -> TransferId {
        let id = TransferId::new();
        let now = Self::now_str();
        let task = TransferTask {
            id,
            session_id: self.session_id,
            direction,
            status: TransferStatus::Queued,
            local_path,
            remote_path,
            transferred_bytes: 0,
            total_bytes,
            started_at: None,
            finished_at: None,
            message: String::new(),
            logs: vec![format!("{now} [INFO] 加入传输队列")],
        };
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.push(task);
        id
    }

    pub fn snapshot(&self) -> Vec<TransferTask> {
        self.tasks.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 取出下一个排队中的任务，标记为 Running
    pub fn dequeue_next(&self) -> Option<TransferTask> {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(idx) = tasks
            .iter()
            .position(|t| t.status == TransferStatus::Queued)
        {
            let mut task = tasks[idx].clone();
            task.status = TransferStatus::Running;
            let now = Self::now_str();
            task.started_at = Some(now.clone());
            task.logs.push(format!("{now} [INFO] 开始传输"));
            tasks[idx] = task.clone();
            Some(task)
        } else {
            None
        }
    }

    /// 判断是否有空闲槽位（Running < MAX_CONCURRENT）
    pub fn has_capacity(&self) -> bool {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let running = tasks
            .iter()
            .filter(|t| t.status == TransferStatus::Running)
            .count();
        running < MAX_CONCURRENT
    }

    pub fn cancel(&self, id: &TransferId) {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = tasks.iter_mut().find(|t| &t.id == id)
            && matches!(t.status, TransferStatus::Queued | TransferStatus::Running)
        {
            t.status = TransferStatus::Cancelled;
            let now = Self::now_str();
            t.logs.push(format!("{now} [WARN] 已取消"));
        }
    }

    pub fn update_progress(&self, id: &TransferId, transferred_bytes: u64, total_bytes: u64) {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = tasks.iter_mut().find(|t| &t.id == id) {
            t.transferred_bytes = transferred_bytes;
            t.total_bytes = total_bytes;
            let now = Self::now_str();
            let pct = if total_bytes > 0 {
                (transferred_bytes as f64 / total_bytes as f64 * 100.0) as u32
            } else {
                0
            };
            t.logs.push(format!(
                "{now} [INFO] 已传输 {} ({pct}%)",
                format_size(transferred_bytes),
            ));
        }
    }

    pub fn mark_running(&self, id: &TransferId) {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = tasks.iter_mut().find(|t| &t.id == id) {
            t.status = TransferStatus::Running;
            t.started_at = Some(Self::now_str());
            let now = Self::now_str();
            t.logs.push(format!("{now} [INFO] 开始传输"));
        }
    }

    pub fn mark_completed(&self, id: &TransferId) {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = tasks.iter_mut().find(|t| &t.id == id) {
            t.status = TransferStatus::Completed;
            t.finished_at = Some(Self::now_str());
            let now = Self::now_str();
            t.logs.push(format!("{now} [INFO] 完成"));
        }
    }

    pub fn mark_failed(&self, id: &TransferId, error: &str) {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = tasks.iter_mut().find(|t| &t.id == id) {
            t.status = TransferStatus::Failed;
            let now = Self::now_str();
            t.logs.push(format!("{now} [ERROR] 失败: {error}"));
        }
    }

    fn now_str() -> String {
        time::OffsetDateTime::now_local()
            .unwrap_or_else(|_| time::OffsetDateTime::now_utc())
            .format(
                &time::format_description::parse("[hour]:[minute]:[second]").expect("时间格式常量"),
            )
            .unwrap_or_else(|_| "00:00:00".into())
    }
}

/// 将 Unix 时间戳或已有字符串格式化为 `MM-DD HH:mm`
pub fn format_modified(raw: &str) -> String {
    if let Ok(secs) = raw.parse::<i64>() {
        let dt = time::OffsetDateTime::from_unix_timestamp(secs)
            .unwrap_or_else(|_| time::OffsetDateTime::UNIX_EPOCH);
        return dt
            .format(
                &time::format_description::parse("[month repr:numerical]-[day] [hour]:[minute]")
                    .expect("时间格式常量"),
            )
            .unwrap_or_else(|_| raw.to_string());
    }
    raw.to_string()
}

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

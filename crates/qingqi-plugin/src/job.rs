#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct JobId(pub String);

impl JobId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Pending | Self::Running)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct JobSnapshot {
    pub id: JobId,
    pub source: &'static str,
    pub title: String,
    pub status: JobStatus,
    pub completed_units: u64,
    pub total_units: Option<u64>,
    pub rate_per_second: f64,
    pub message: String,
}

impl JobSnapshot {
    pub fn progress(&self) -> Option<f64> {
        let total = self.total_units?;
        if total == 0 {
            return None;
        }
        Some((self.completed_units as f64 / total as f64).clamp(0.0, 1.0))
    }
}

pub trait JobProvider {
    fn job_snapshots(&self) -> Vec<JobSnapshot>;
    fn cancel_job(&self, id: &JobId) -> anyhow::Result<()>;
    fn pause_job(&self, id: &JobId) -> anyhow::Result<()>;
    fn resume_job(&self, id: &JobId) -> anyhow::Result<()>;
}

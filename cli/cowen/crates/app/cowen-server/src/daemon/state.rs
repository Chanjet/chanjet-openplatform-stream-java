use tokio::time::Instant;
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use tokio::task::JoinHandle;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum WorkerStatus {
    Created,
    Starting,
    Running,
    Backoff { 
        retry_count: u32, 
        #[serde(skip)]
        next_retry_at: Instant,
        last_error: String 
    },
    Failed { reason: String },
    Draining,
    Stopped,
}

pub struct ProfileWorker {
    pub profile: String,
    pub status: WorkerStatus,
    pub cancel_token: CancellationToken,
    pub join_handle: Option<JoinHandle<()>>,
}

impl ProfileWorker {
    pub fn new(profile: &str) -> Self {
        Self {
            profile: profile.to_string(),
            status: WorkerStatus::Created,
            cancel_token: CancellationToken::new(),
            join_handle: None,
        }
    }

    pub fn is_active(&self) -> bool {
        match self.status {
            WorkerStatus::Starting | WorkerStatus::Running | WorkerStatus::Backoff { .. } | WorkerStatus::Draining => true,
            _ => false,
        }
    }

    pub fn can_start(&self) -> bool {
        match self.status {
            WorkerStatus::Created | WorkerStatus::Stopped | WorkerStatus::Failed { .. } => true,
            _ => false,
        }
    }

    pub fn can_stop(&self) -> bool {
        match self.status {
            WorkerStatus::Starting | WorkerStatus::Running | WorkerStatus::Backoff { .. } => true,
            _ => false,
        }
    }
}

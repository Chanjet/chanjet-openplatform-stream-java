use serde::Serialize;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum WorkerStatus {
    Created,
    Starting,
    Running,
    Backoff {
        retry_count: u32,
        #[serde(skip)]
        next_retry_at: Instant,
        last_error: String,
    },
    Failed {
        reason: String,
    },
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
        matches!(
            self.status,
            WorkerStatus::Starting
                | WorkerStatus::Running
                | WorkerStatus::Backoff { .. }
                | WorkerStatus::Draining
        )
    }

    pub fn can_start(&self) -> bool {
        matches!(
            self.status,
            WorkerStatus::Created | WorkerStatus::Stopped | WorkerStatus::Failed { .. }
        )
    }

    pub fn can_stop(&self) -> bool {
        matches!(
            self.status,
            WorkerStatus::Starting | WorkerStatus::Running | WorkerStatus::Backoff { .. }
        )
    }
}

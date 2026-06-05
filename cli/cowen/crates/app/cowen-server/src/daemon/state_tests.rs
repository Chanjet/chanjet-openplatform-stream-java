#[cfg(test)]
mod tests {
    use crate::daemon::state::{ProfileWorker, WorkerStatus};
    use tokio::time::Instant;

    #[test]
    fn test_worker_can_start() {
        let mut worker = ProfileWorker::new("test");
        assert!(worker.can_start());

        worker.status = WorkerStatus::Running;
        assert!(!worker.can_start());

        worker.status = WorkerStatus::Stopped;
        assert!(worker.can_start());

        worker.status = WorkerStatus::Failed { reason: "test".into() };
        assert!(worker.can_start());

        worker.status = WorkerStatus::Starting;
        assert!(!worker.can_start());
    }

    #[test]
    fn test_worker_can_stop() {
        let mut worker = ProfileWorker::new("test");
        assert!(!worker.can_stop());

        worker.status = WorkerStatus::Starting;
        assert!(worker.can_stop());

        worker.status = WorkerStatus::Running;
        assert!(worker.can_stop());

        worker.status = WorkerStatus::Backoff { 
            retry_count: 1, 
            next_retry_at: Instant::now(), 
            last_error: "".into() 
        };
        assert!(worker.can_stop());

        worker.status = WorkerStatus::Stopped;
        assert!(!worker.can_stop());
    }
}

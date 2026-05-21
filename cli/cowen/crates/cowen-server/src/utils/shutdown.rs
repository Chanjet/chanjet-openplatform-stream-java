use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// `ShutdownGate` is responsible for tracking active asynchronous tasks.
/// It acts as a gatekeeper during the graceful shutdown process,
/// ensuring that the system waits for all currently executing tasks
/// (e.g., webhook retries, token refreshes) to complete before
/// shutting down completely.
#[derive(Debug, Clone)]
pub struct ShutdownGate {
    /// Atomic counter for the number of currently active tasks.
    active_tasks: Arc<AtomicUsize>,
    /// Notification mechanism used to signal when `active_tasks` reaches zero.
    zero_notify: Arc<Notify>,
}

impl ShutdownGate {
    /// Creates a new `ShutdownGate`.
    pub fn new() -> Self {
        Self {
            active_tasks: Arc::new(AtomicUsize::new(0)),
            zero_notify: Arc::new(Notify::new()),
        }
    }

    /// Enters the gate, indicating that a new task has started.
    /// This should be called before an asynchronous task begins its core logic.
    /// Returns a `GateGuard` which will automatically decrement the count when dropped.
    pub fn enter(&self) -> GateGuard {
        self.active_tasks.fetch_add(1, Ordering::SeqCst);
        GateGuard {
            active_tasks: self.active_tasks.clone(),
            zero_notify: self.zero_notify.clone(),
        }
    }

    /// Waits asynchronously until the number of active tasks drops to zero.
    /// This is typically called during the shutdown phase after refusing new tasks.
    pub async fn wait_for_zero(&self) {
        // We use a loop to avoid missed wakeups (though Notify handles this well,
        // it's good practice when checking atomic state).
        while self.active_tasks.load(Ordering::SeqCst) > 0 {
            self.zero_notify.notified().await;
        }
    }
    
    /// Returns the current number of active tasks.
    /// Useful for logging and monitoring during shutdown.
    pub fn active_count(&self) -> usize {
        self.active_tasks.load(Ordering::SeqCst)
    }
}

/// A RAII guard for an active task.
/// When this guard is dropped, it automatically decrements the task count
/// in the associated `ShutdownGate` and signals `zero_notify` if the count reaches zero.
pub struct GateGuard {
    active_tasks: Arc<AtomicUsize>,
    zero_notify: Arc<Notify>,
}

impl Drop for GateGuard {
    fn drop(&mut self) {
        let prev = self.active_tasks.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            // If the previous value was 1, it means the count just became 0.
            self.zero_notify.notify_waiters();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_shutdown_gate_basic() {
        let gate = ShutdownGate::new();
        assert_eq!(gate.active_count(), 0);

        let guard1 = gate.enter();
        assert_eq!(gate.active_count(), 1);

        let guard2 = gate.enter();
        assert_eq!(gate.active_count(), 2);

        drop(guard1);
        assert_eq!(gate.active_count(), 1);

        drop(guard2);
        assert_eq!(gate.active_count(), 0);
    }

    #[tokio::test]
    async fn test_shutdown_gate_wait() {
        let gate = ShutdownGate::new();
        let gate_clone = gate.clone();

        let guard = gate.enter();

        let wait_task = tokio::spawn(async move {
            gate_clone.wait_for_zero().await;
        });

        // Ensure the wait_task is actually waiting
        sleep(Duration::from_millis(50)).await;
        assert!(!wait_task.is_finished());

        // Drop the guard, which should unblock wait_task
        drop(guard);

        // wait_task should now complete
        let _ = tokio::time::timeout(Duration::from_millis(100), wait_task)
            .await
            .expect("Task did not complete in time")
            .expect("Join error");
    }
}

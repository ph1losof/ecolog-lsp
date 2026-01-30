//! Background task management with cancellation support.
//!
//! This module provides utilities for managing background tasks with proper
//! cancellation and shutdown semantics.

use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Token for cooperative cancellation of background tasks.
///
/// Tasks should periodically check `is_cancelled()` and exit early if true.
#[derive(Clone)]
pub struct CancellationToken {
    sender: Arc<broadcast::Sender<()>>,
}

impl CancellationToken {
    /// Create a new cancellation token.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1);
        Self {
            sender: Arc::new(sender),
        }
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        // If there are no receivers, the channel is effectively cancelled
        self.sender.receiver_count() == 0 && !self.sender.is_empty()
    }

    /// Cancel all tasks associated with this token.
    pub fn cancel(&self) {
        // Ignore error if no receivers
        let _ = self.sender.send(());
    }

    /// Subscribe to cancellation notifications.
    ///
    /// Returns a receiver that will receive a message when cancellation is requested.
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.sender.subscribe()
    }

    /// Wait for cancellation to be signaled.
    pub async fn cancelled(&self) {
        let mut receiver = self.subscribe();
        let _ = receiver.recv().await;
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Manager for background tasks with graceful shutdown support.
///
/// Provides a centralized way to spawn and manage background tasks,
/// ensuring they can be cleanly shut down when the server stops.
pub struct BackgroundTaskManager {
    token: CancellationToken,
    tasks: Vec<JoinHandle<()>>,
}

impl BackgroundTaskManager {
    /// Create a new task manager.
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
            tasks: Vec::new(),
        }
    }

    /// Get the cancellation token for this manager.
    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// Spawn a new background task.
    ///
    /// The task should periodically check the cancellation token and exit early.
    pub fn spawn<F>(&mut self, name: &'static str, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        debug!("Spawning background task: {}", name);
        let handle = tokio::spawn(async move {
            fut.await;
            debug!("Background task completed: {}", name);
        });
        self.tasks.push(handle);
    }

    /// Spawn a task that automatically handles cancellation.
    ///
    /// The provided future will be raced against the cancellation signal.
    pub fn spawn_cancellable<F>(&mut self, name: &'static str, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let token = self.token.clone();
        debug!("Spawning cancellable background task: {}", name);

        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = token.cancelled() => {
                    debug!("Background task cancelled: {}", name);
                }
                _ = fut => {
                    debug!("Background task completed: {}", name);
                }
            }
        });
        self.tasks.push(handle);
    }

    /// Request cancellation of all managed tasks.
    pub fn cancel(&self) {
        info!("Cancelling all background tasks");
        self.token.cancel();
    }

    /// Gracefully shutdown all managed tasks.
    ///
    /// Requests cancellation and then waits for all tasks to complete,
    /// with an optional timeout.
    pub async fn shutdown(self) {
        info!("Shutting down background task manager");
        self.token.cancel();

        for (i, handle) in self.tasks.into_iter().enumerate() {
            match handle.await {
                Ok(()) => debug!("Task {} completed", i),
                Err(e) => warn!("Task {} panicked: {}", i, e),
            }
        }

        info!("All background tasks completed");
    }

    /// Number of managed tasks.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

impl Default for BackgroundTaskManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn test_cancellation_token() {
        let token = CancellationToken::new();
        let token2 = token.clone();

        let completed = Arc::new(AtomicBool::new(false));
        let completed2 = completed.clone();

        tokio::spawn(async move {
            token2.cancelled().await;
            completed2.store(true, Ordering::SeqCst);
        });

        // Give the task time to start
        tokio::time::sleep(Duration::from_millis(10)).await;

        assert!(!completed.load(Ordering::SeqCst));

        token.cancel();

        // Give the task time to respond
        tokio::time::sleep(Duration::from_millis(10)).await;

        assert!(completed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_background_task_manager() {
        let mut manager = BackgroundTaskManager::new();

        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter2 = counter.clone();

        manager.spawn("test_task", async move {
            counter2.fetch_add(1, Ordering::SeqCst);
        });

        assert_eq!(manager.task_count(), 1);

        // Allow task to run
        tokio::time::sleep(Duration::from_millis(50)).await;

        manager.shutdown().await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_cancellable_task() {
        let mut manager = BackgroundTaskManager::new();

        let running = Arc::new(AtomicBool::new(false));
        let running2 = running.clone();

        manager.spawn_cancellable("long_running", async move {
            running2.store(true, Ordering::SeqCst);
            // This would run forever if not cancelled
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });

        // Give task time to start
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(running.load(Ordering::SeqCst));

        // Shutdown should cancel the infinite loop
        manager.shutdown().await;
    }
}

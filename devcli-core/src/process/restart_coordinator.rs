// Restart coordinator to prevent race conditions between monitor restarts and manual restarts
// Ensures only one restart operation is in progress for a given app at a time

use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Lock to prevent duplicate restarts
/// When a restart is in progress, this prevents another restart from starting
/// This is just a marker type - the existence of the entry in the HashMap is the lock
struct RestartLock;

/// Guard that releases the restart lock when dropped
pub struct RestartGuard {
    app_key: String,
    coordinator: Arc<RestartCoordinator>,
}

impl Drop for RestartGuard {
    fn drop(&mut self) {
        // Release the lock synchronously
        // This is safe because we're using std::sync::Mutex
        let mut locks = self.coordinator.locks.lock()
            .expect("RestartCoordinator mutex poisoned");
        locks.remove(&self.app_key);
    }
}

/// Coordinates restart operations to prevent duplicates
pub struct RestartCoordinator {
    locks: Arc<Mutex<HashMap<String, RestartLock>>>,
}

impl RestartCoordinator {
    /// Create a new restart coordinator
    pub fn new() -> Self {
        Self {
            locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Try to acquire a restart lock for an app
    /// Returns Some(guard) if lock was acquired, None if restart already in progress
    pub async fn try_acquire_restart_lock(&self, app_key: &str) -> Option<RestartGuard> {
        let mut locks = self.locks.lock()
            .expect("RestartCoordinator mutex poisoned");

        // Check if restart already in progress
        if locks.contains_key(app_key) {
            return None;
        }

        // Acquire the lock
        locks.insert(app_key.to_string(), RestartLock);

        Some(RestartGuard {
            app_key: app_key.to_string(),
            coordinator: Arc::new(Self {
                locks: self.locks.clone(),
            }),
        })
    }

    /// Check if a restart is currently in progress for an app
    pub async fn is_restart_in_progress(&self, app_key: &str) -> bool {
        let locks = self.locks.lock()
            .expect("RestartCoordinator mutex poisoned");
        locks.contains_key(app_key)
    }

    /// Execute a restart with coordination
    /// This is a convenience method that handles locking, backoff, and execution
    pub async fn execute_restart_if_allowed(
        &self,
        app_key: &str,
        backoff_duration: std::time::Duration,
        restart_fn: impl std::future::Future<Output = Result<()>>,
    ) -> Result<bool> {
        // Try to acquire lock
        let guard = match self.try_acquire_restart_lock(app_key).await {
            Some(guard) => guard,
            None => {
                // Restart already in progress
                return Ok(false);
            }
        };

        // Apply backoff delay
        tokio::time::sleep(backoff_duration).await;

        // Execute the restart
        let result = restart_fn.await;

        // Guard is dropped here, releasing the lock
        drop(guard);

        // Return success/failure
        result.map(|_| true)
    }
}

impl Default for RestartCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_acquire_lock_success() {
        let coordinator = RestartCoordinator::new();
        let guard = coordinator.try_acquire_restart_lock("test-app").await;
        assert!(guard.is_some());
    }

    #[tokio::test]
    async fn test_acquire_lock_prevents_duplicate() {
        let coordinator = RestartCoordinator::new();
        let _guard1 = coordinator.try_acquire_restart_lock("test-app").await;
        let guard2 = coordinator.try_acquire_restart_lock("test-app").await;
        assert!(guard2.is_none());
    }

    #[tokio::test]
    async fn test_lock_released_on_drop() {
        let coordinator = RestartCoordinator::new();
        {
            let _guard = coordinator.try_acquire_restart_lock("test-app").await;
            assert!(coordinator.is_restart_in_progress("test-app").await);
        }
        // Guard dropped, lock should be released
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert!(!coordinator.is_restart_in_progress("test-app").await);
    }

    #[tokio::test]
    async fn test_execute_restart_with_lock() {
        let coordinator = RestartCoordinator::new();
        let executed = Arc::new(Mutex::new(false));
        let executed_clone = executed.clone();

        let result = coordinator
            .execute_restart_if_allowed(
                "test-app",
                std::time::Duration::from_millis(10),
                async move {
                    let mut flag = executed_clone.lock()
                        .expect("Test mutex poisoned");
                    *flag = true;
                    Ok(())
                },
            )
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert!(*executed.lock().expect("Test mutex poisoned"));
    }

    #[tokio::test]
    async fn test_execute_restart_blocked_by_existing_lock() {
        let coordinator = RestartCoordinator::new();
        let _guard = coordinator.try_acquire_restart_lock("test-app").await;

        let executed = Arc::new(Mutex::new(false));
        let executed_clone = executed.clone();

        let result = coordinator
            .execute_restart_if_allowed(
                "test-app",
                std::time::Duration::from_millis(10),
                async move {
                    let mut flag = executed_clone.lock()
                        .expect("Test mutex poisoned");
                    *flag = true;
                    Ok(())
                },
            )
            .await;

        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false (not executed)
        assert!(!*executed.lock().expect("Test mutex poisoned")); // Should not have executed
    }
}

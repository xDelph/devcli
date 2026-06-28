#[cfg(test)]
mod tests {
    use crate::restart::RestartCoordinator;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_try_acquire_exclusive_lock() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = RestartCoordinator::new(temp_dir.path().to_path_buf());

        let guard1 = coordinator.try_acquire("app-1").unwrap();
        assert!(guard1.is_some());

        let guard2 = coordinator.try_acquire("app-1").unwrap();
        assert!(guard2.is_none());

        drop(guard1);

        let guard3 = coordinator.try_acquire("app-1").unwrap();
        assert!(guard3.is_some());
    }

    #[tokio::test]
    async fn test_execute_if_allowed_runs_once() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = Arc::new(RestartCoordinator::new(temp_dir.path().to_path_buf()));

        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        let executed = coordinator
            .clone()
            .execute_if_allowed("app-1", Duration::ZERO, move || {
                let counter = Arc::clone(&counter_clone);
                async move {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                }
            })
            .await
            .unwrap();

        assert!(executed);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_execute_if_allowed_skips_when_locked() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = Arc::new(RestartCoordinator::new(temp_dir.path().to_path_buf()));

        let _guard = coordinator.try_acquire("app-1").unwrap().unwrap();

        let executed = coordinator
            .execute_if_allowed("app-1", Duration::ZERO, || async { Ok(()) })
            .await
            .unwrap();

        assert!(!executed);
    }
}

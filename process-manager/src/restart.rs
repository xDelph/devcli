use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum RestartReason {
    Crash { exit_code: i32 },
    HealthCheckFailure { check_type: String },
    Manual,
}

pub struct RestartGuard {
    file: File,
}

impl Drop for RestartGuard {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub struct RestartCoordinator {
    base_dir: PathBuf,
}

impl RestartCoordinator {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn try_acquire(&self, id: &str) -> Result<Option<RestartGuard>> {
        std::fs::create_dir_all(&self.base_dir)?;
        let lock_path = self.base_dir.join(format!("{}.lock", id));

        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false) // We don't want to truncate, just use the file as a lock
            .open(&lock_path)
            .context("Failed to open lock file")?;

        match file.try_lock_exclusive() {
            Ok(_) => Ok(Some(RestartGuard { file })),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn execute_if_allowed<F, Fut>(
        self: Arc<Self>,
        id: &str,
        backoff: Duration,
        restart_fn: F,
    ) -> Result<bool>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let guard = match self.try_acquire(id)? {
            Some(g) => g,
            None => return Ok(false),
        };

        if !backoff.is_zero() {
            tokio::time::sleep(backoff).await;
        }

        restart_fn().await?;

        drop(guard);

        Ok(true)
    }
}

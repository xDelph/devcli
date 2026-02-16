//! Async YAML configuration loader implementation.

use crate::core::{Error, Result};
use crate::loader::AsyncConfigLoader;
use crate::utils::path::expand_all;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Async YAML file-based configuration loader.
#[derive(Debug, Clone)]
pub struct AsyncYamlLoader {
    path: PathBuf,
    expand_paths: bool,
    create_dirs: bool,
}

impl AsyncYamlLoader {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            expand_paths: true,
            create_dirs: true,
        }
    }

    pub fn with_path_expansion(mut self, enabled: bool) -> Self {
        self.expand_paths = enabled;
        self
    }

    pub fn with_create_dirs(mut self, enabled: bool) -> Self {
        self.create_dirs = enabled;
        self
    }

    fn expanded_path(&self) -> PathBuf {
        if self.expand_paths {
            expand_all(&self.path)
        } else {
            self.path.clone()
        }
    }
}

#[async_trait]
impl<T> AsyncConfigLoader<T> for AsyncYamlLoader
where
    T: DeserializeOwned + Serialize + Send + Sync,
{
    async fn load(&self) -> Result<T> {
        let path = self.expanded_path();

        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Err(Error::LoadError(format!(
                "Configuration file not found: {}",
                path.display()
            )));
        }

        let content = fs::read_to_string(&path)
            .await
            .map_err(|e| Error::LoadError(format!("Failed to read {}: {}", path.display(), e)))?;

        serde_yaml::from_str(&content).map_err(|e| {
            Error::LoadError(format!(
                "Failed to parse YAML from {}: {}",
                path.display(),
                e
            ))
        })
    }

    async fn save(&self, config: &T) -> Result<()> {
        let path = self.expanded_path();

        if self.create_dirs {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await.map_err(|e| {
                    Error::SaveError(format!(
                        "Failed to create directory {}: {}",
                        parent.display(),
                        e
                    ))
                })?;
            }
        }

        let content = serde_yaml::to_string(config)
            .map_err(|e| Error::SaveError(format!("Failed to serialize YAML: {}", e)))?;

        fs::write(&path, content).await.map_err(|e| {
            Error::SaveError(format!("Failed to write to {}: {}", path.display(), e))
        })?;

        Ok(())
    }

    fn exists(&self) -> bool {
        self.expanded_path().exists()
    }

    fn source_info(&self) -> String {
        self.path.display().to_string()
    }
}

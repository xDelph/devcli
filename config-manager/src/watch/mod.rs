//! Configuration file watching and hot-reload support.
//!
//! This module provides functionality for watching configuration files
//! and automatically reloading when changes are detected.

use crate::core::{Error, Result};
use crate::loader::ConfigLoader;
use crossbeam_channel::{bounded, Receiver, Sender};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Configuration change event.
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    /// Configuration was modified
    Modified,
    /// Configuration was created
    Created,
    /// Configuration was deleted
    Removed,
    /// Error occurred while watching
    Error(String),
}

/// Configuration watcher that monitors file changes.
///
/// Watches a configuration file and triggers callbacks when changes are detected.
///
/// # Examples
///
/// ```rust,no_run
/// use config_manager::prelude::*;
/// use config_manager::watch::{ConfigWatcher, ConfigEvent};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct MyConfig {
///     name: String,
///     port: u16,
/// }
///
/// fn main() -> Result<()> {
///     let loader = JsonLoader::new("config.json");
///     let mut watcher: ConfigWatcher<MyConfig> = ConfigWatcher::new(loader)?;
///
///     // Start watching
///     watcher.start(|event| {
///         match event {
///             ConfigEvent::Modified => println!("Config was modified!"),
///             ConfigEvent::Created => println!("Config was created!"),
///             ConfigEvent::Removed => println!("Config was removed!"),
///             ConfigEvent::Error(e) => eprintln!("Watch error: {}", e),
///         }
///     })?;
///
///     // Keep watching...
///     std::thread::sleep(std::time::Duration::from_secs(60));
///
///     watcher.stop()?;
///     Ok(())
/// }
/// ```
pub struct ConfigWatcher<T>
where
    T: Send + Sync + 'static,
{
    loader: Box<dyn ConfigLoader<T>>,
    path: PathBuf,
    watcher: Option<RecommendedWatcher>,
    event_tx: Option<Sender<ConfigEvent>>,
    event_rx: Option<Receiver<ConfigEvent>>,
    is_watching: Arc<Mutex<bool>>,
}

impl<T> ConfigWatcher<T>
where
    T: Send + Sync + 'static,
{
    /// Create a new configuration watcher.
    ///
    /// # Arguments
    ///
    /// * `loader` - The configuration loader to watch
    ///
    /// # Errors
    ///
    /// Returns an error if the loader's source is not file-based.
    pub fn new(loader: impl ConfigLoader<T> + 'static) -> Result<Self> {
        let source_info = loader.source_info();
        let path = PathBuf::from(source_info);

        // Validate that this is a file path
        if !path.is_absolute() && !path.exists() && !path.starts_with("~") && !path.starts_with(".")
        {
            return Err(Error::Custom(
                "ConfigWatcher requires a file-based loader with a valid path".to_string(),
            ));
        }

        let (event_tx, event_rx) = bounded(100);

        Ok(Self {
            loader: Box::new(loader),
            path,
            watcher: None,
            event_tx: Some(event_tx),
            event_rx: Some(event_rx),
            is_watching: Arc::new(Mutex::new(false)),
        })
    }

    /// Start watching the configuration file.
    ///
    /// Spawns a background thread that monitors file changes and calls
    /// the provided callback when events occur.
    ///
    /// # Arguments
    ///
    /// * `callback` - Function to call when configuration changes
    ///
    /// # Errors
    ///
    /// Returns an error if watching fails to start.
    pub fn start<F>(&mut self, callback: F) -> Result<()>
    where
        F: Fn(ConfigEvent) + Send + 'static,
    {
        let mut is_watching = self
            .is_watching
            .lock()
            .map_err(|e| Error::Custom(format!("Lock error: {}", e)))?;

        if *is_watching {
            return Err(Error::Custom("Watcher is already running".to_string()));
        }

        let event_tx = self
            .event_tx
            .clone()
            .ok_or_else(|| Error::Custom("Event channel not initialized".to_string()))?;

        // Create the notify watcher
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            let event = match res {
                Ok(event) => match event.kind {
                    EventKind::Modify(_) => ConfigEvent::Modified,
                    EventKind::Create(_) => ConfigEvent::Created,
                    EventKind::Remove(_) => ConfigEvent::Removed,
                    _ => return,
                },
                Err(e) => ConfigEvent::Error(e.to_string()),
            };

            let _ = event_tx.send(event);
        })
        .map_err(|e| Error::Custom(format!("Failed to create watcher: {}", e)))?;

        // Watch the file or its parent directory
        let watch_path = if self.path.exists() {
            &self.path
        } else if let Some(parent) = self.path.parent() {
            parent
        } else {
            return Err(Error::Custom("Cannot determine path to watch".to_string()));
        };

        watcher
            .watch(watch_path, RecursiveMode::NonRecursive)
            .map_err(|e| Error::Custom(format!("Failed to watch path: {}", e)))?;

        self.watcher = Some(watcher);
        *is_watching = true;

        // Spawn thread to handle events
        let event_rx = self
            .event_rx
            .take()
            .ok_or_else(|| Error::Custom("Event receiver not initialized".to_string()))?;

        let is_watching_clone = Arc::clone(&self.is_watching);

        thread::spawn(move || {
            loop {
                // Check if we should stop
                {
                    let watching = is_watching_clone.lock().unwrap();
                    if !*watching {
                        break;
                    }
                }

                // Wait for events with timeout
                match event_rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(event) => callback(event),
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        Ok(())
    }

    /// Stop watching the configuration file.
    ///
    /// # Errors
    ///
    /// Returns an error if the watcher is not running.
    pub fn stop(&mut self) -> Result<()> {
        let mut is_watching = self
            .is_watching
            .lock()
            .map_err(|e| Error::Custom(format!("Lock error: {}", e)))?;

        if !*is_watching {
            return Err(Error::Custom("Watcher is not running".to_string()));
        }

        *is_watching = false;
        self.watcher = None;

        Ok(())
    }

    /// Check if the watcher is currently running.
    pub fn is_watching(&self) -> bool {
        self.is_watching.lock().map(|w| *w).unwrap_or(false)
    }

    /// Get the watched file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Manually reload the configuration.
    ///
    /// This can be called independently of file watching.
    ///
    /// # Errors
    ///
    /// Returns an error if loading fails.
    pub fn reload(&self) -> Result<T> {
        self.loader.load()
    }
}

impl<T> Drop for ConfigWatcher<T>
where
    T: Send + Sync + 'static,
{
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::JsonLoader;
    use serde::{Deserialize, Serialize};
    use std::fs;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestConfig {
        name: String,
        value: u32,
    }

    #[test]
    fn test_watcher_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let loader = JsonLoader::new(&config_path);
        let watcher: ConfigWatcher<TestConfig> = ConfigWatcher::new(loader).unwrap();

        assert!(!watcher.is_watching());
        assert_eq!(watcher.path(), config_path);
    }

    #[test]
    fn test_watcher_start_stop() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        // Create initial config
        let config = TestConfig {
            name: "test".to_string(),
            value: 42,
        };
        let loader = JsonLoader::new(&config_path);
        loader.save(&config).unwrap();

        let mut watcher: ConfigWatcher<TestConfig> = ConfigWatcher::new(loader).unwrap();

        // Start watching
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);

        watcher
            .start(move |event| {
                events_clone.lock().unwrap().push(event);
            })
            .unwrap();

        assert!(watcher.is_watching());

        // Stop watching
        watcher.stop().unwrap();
        assert!(!watcher.is_watching());
    }

    #[test]
    fn test_watcher_detects_modification() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        // Create initial config
        let config = TestConfig {
            name: "test".to_string(),
            value: 42,
        };
        fs::write(&config_path, serde_json::to_string(&config).unwrap()).unwrap();

        let loader = JsonLoader::new(&config_path);
        let mut watcher: ConfigWatcher<TestConfig> = ConfigWatcher::new(loader).unwrap();

        // Start watching
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);

        watcher
            .start(move |event| {
                events_clone.lock().unwrap().push(event);
            })
            .unwrap();

        // Give watcher time to initialize
        thread::sleep(Duration::from_millis(100));

        // Modify the file
        let new_config = TestConfig {
            name: "modified".to_string(),
            value: 100,
        };
        fs::write(&config_path, serde_json::to_string(&new_config).unwrap()).unwrap();

        // Wait for event to be processed
        thread::sleep(Duration::from_millis(500));

        // Check that we received a modification event
        let events = events.lock().unwrap();
        let has_modified = events.iter().any(|e| matches!(e, ConfigEvent::Modified));

        assert!(has_modified, "Expected at least one Modified event");

        watcher.stop().unwrap();
    }

    #[test]
    fn test_watcher_manual_reload() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        // Create config
        let config = TestConfig {
            name: "test".to_string(),
            value: 42,
        };
        let loader = JsonLoader::new(&config_path);
        loader.save(&config).unwrap();

        let watcher: ConfigWatcher<TestConfig> = ConfigWatcher::new(loader).unwrap();

        // Manually reload
        let loaded = watcher.reload().unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn test_watcher_double_start_error() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let config = TestConfig {
            name: "test".to_string(),
            value: 42,
        };
        let loader = JsonLoader::new(&config_path);
        loader.save(&config).unwrap();

        let mut watcher: ConfigWatcher<TestConfig> = ConfigWatcher::new(loader).unwrap();

        watcher.start(|_| {}).unwrap();

        // Try to start again
        let result = watcher.start(|_| {});
        assert!(result.is_err());

        watcher.stop().unwrap();
    }
}

// Configuration module - Manages loading, saving, and resolving app configurations
// 
// This module contains:
// - models: Data structures for config.json and preferences.json
// - loader: Functions to read/write configuration files
// - resolver: Logic to find apps by name and resolve dependencies
// - dependencies: Dependency graph resolution for app startup

pub mod dependencies;
pub mod loader;
pub mod models;
pub mod resolver;

#[cfg(test)]
mod config_tests;
#[cfg(test)]
mod tests;

pub use loader::{load_config, load_preferences, save_config, save_preferences};
pub use models::{App, Commands, Config, Defaults, Dependency, Preferences, Project};
pub use resolver::{get_app_by_project, list_all_apps, resolve_app};


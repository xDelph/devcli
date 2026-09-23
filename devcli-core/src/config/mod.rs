// Configuration module - Manages loading, saving, and resolving app configurations
//
// JSON I/O, resolution, dependency graphs, and validation rules are backed by
// config-manager via `crate::config_manager_support`. This module keeps domain
// types and the public CLI-facing API.

pub mod dependencies;
pub mod loader;
pub mod models;
pub mod resolver;

#[cfg(test)]
mod config_tests;
#[cfg(test)]
mod tests;

pub use loader::{load_config, load_preferences, save_config, save_preferences};
pub use models::{App, Commands, Config, Defaults, Dependency, Environment, Preferences, Project};
pub use resolver::{get_app_by_project, list_all_apps, resolve_app};

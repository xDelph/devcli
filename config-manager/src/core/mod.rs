//! Core configuration management types and orchestration.

pub mod builder;
pub mod error;
pub mod manager;

pub use builder::ConfigManagerBuilder;
pub use error::{Error, Result};
pub use manager::ConfigManager;

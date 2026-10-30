//! Configuration loading and saving.
//!
//! This module provides traits and implementations for loading and saving
//! configurations from various sources (files, environment variables, etc.).

mod json;
mod traits;

pub use json::JsonLoader;
pub use traits::ConfigLoader;

#[cfg(feature = "toml")]
pub mod toml_loader;
#[cfg(feature = "toml")]
pub use toml_loader::TomlLoader;

#[cfg(feature = "yaml")]
pub mod yaml_loader;
#[cfg(feature = "yaml")]
pub use yaml_loader::YamlLoader;

mod layered;
pub use layered::{LayeredLoader, MergeStrategy};

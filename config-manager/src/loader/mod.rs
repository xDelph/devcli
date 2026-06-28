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

mod async_traits;
pub use async_traits::AsyncConfigLoader;

mod async_json;
pub use async_json::AsyncJsonLoader;

#[cfg(feature = "toml")]
mod async_toml;
#[cfg(feature = "toml")]
pub use async_toml::AsyncTomlLoader;

#[cfg(feature = "yaml")]
mod async_yaml;
#[cfg(feature = "yaml")]
pub use async_yaml::AsyncYamlLoader;

#[cfg(feature = "http")]
mod http;
#[cfg(feature = "http")]
pub use http::{HttpAuth, HttpLoader};

#[cfg(feature = "encryption")]
mod encrypted;
#[cfg(feature = "encryption")]
pub use encrypted::EncryptedLoader;

#[cfg(feature = "s3")]
mod s3;
#[cfg(feature = "s3")]
pub use s3::S3Loader;

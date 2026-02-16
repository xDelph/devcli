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

#[cfg(feature = "async")]
mod async_traits;
#[cfg(feature = "async")]
pub use async_traits::AsyncConfigLoader;

#[cfg(feature = "async")]
mod async_json;
#[cfg(feature = "async")]
pub use async_json::AsyncJsonLoader;

#[cfg(all(feature = "async", feature = "toml"))]
mod async_toml;
#[cfg(all(feature = "async", feature = "toml"))]
pub use async_toml::AsyncTomlLoader;

#[cfg(all(feature = "async", feature = "yaml"))]
mod async_yaml;
#[cfg(all(feature = "async", feature = "yaml"))]
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

//! Built-in detection strategies
//!
//! Strategies are organized into two categories:
//! - **App Types**: Detect WHAT the application is (language, framework, service, etc.)
//! - **Environment Capabilities**: Detect HOW the application can run (local, docker, k8s, etc.)

// App Type Strategies
pub mod app_types;

// Environment Capability Strategies
pub mod env_capabilities;

// Re-export all strategies for convenience
pub use app_types::*;
pub use env_capabilities::*;

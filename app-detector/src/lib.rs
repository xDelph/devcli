//! # app-detector
//!
//! A strategy-based detection platform for automatically discovering application types,
//! frameworks, and deployment environments.
//!
//! ## Quick Start
//!
//! ```
//! use app_detector::{DetectionEngine, StrategyRegistry};
//! use tempfile::TempDir;
//! use std::fs;
//!
//! // Create a test Rust project
//! let temp = TempDir::new().unwrap();
//! fs::write(temp.path().join("Cargo.toml"),
//!     "[package]\nname = \"test\"\nversion = \"1.0.0\"\nedition = \"2021\"\n").unwrap();
//!
//! let registry = StrategyRegistry::with_defaults();
//! let engine = DetectionEngine::new(registry);
//! let report = engine.detect(temp.path()).unwrap();
//!
//! if let Some(lang) = report.primary_language() {
//!     println!("Detected: {}", lang.strategy_id);
//!     assert_eq!(lang.strategy_id, "rust");
//! }
//! ```

pub mod context;
pub mod engine;
pub mod graph;
pub mod registry;
pub mod strategy;
pub mod types;
pub mod utils;

// Built-in strategies
pub mod strategies;

// Re-export main types
pub use context::DetectionContext;
pub use engine::{DetectionEngine, DetectionConfig};
pub use registry::StrategyRegistry;
pub use strategy::DetectionStrategy;
pub use types::*;

// Convenience type alias
pub type Result<T> = anyhow::Result<T>;

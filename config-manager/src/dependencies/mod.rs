//! Dependency resolution and graph management.
//!
//! This module provides traits and implementations for managing dependencies
//! between entities, including cycle detection and topological sorting.

mod graph;
mod traits;

pub use graph::DependencyGraph;
pub use traits::DependencyProvider;

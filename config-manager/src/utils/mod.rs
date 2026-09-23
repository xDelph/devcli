//! Utility functions and helpers.
//!
//! This module provides various utility functions used throughout the library.

pub mod path;
pub mod string;

// Re-export commonly used utilities
pub use path::{contract_tilde, expand_all, expand_env_vars, expand_path, expand_tilde};
pub use string::{find_closest_match, levenshtein_distance};

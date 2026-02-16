//! Entity resolution with fuzzy matching.
//!
//! This module provides traits and implementations for resolving entities
//! by identifier, with support for typo detection and ambiguity handling.

mod fuzzy;
mod hashmap;
mod traits;

pub use fuzzy::fuzzy_match;
pub use hashmap::HashMapResolver;
pub use traits::Resolver;

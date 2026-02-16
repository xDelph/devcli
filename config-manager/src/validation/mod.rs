//! Configuration validation.
//!
//! This module provides traits and implementations for validating
//! configurations with custom rules and constraints.

mod result;
mod traits;

pub use result::{ValidationError, ValidationResult, ValidationWarning};
pub use traits::Validator;

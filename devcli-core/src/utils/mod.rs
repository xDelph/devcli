//! General utility modules
//!
//! Contains helper functions for:
//! - `app`: Application-related utilities (e.g., environment listing)
//! - `command`: Command execution helpers
//! - `path`: Path manipulation and expansion

pub mod app;
pub mod command;
pub mod path;

#[cfg(test)]
mod command_tests;

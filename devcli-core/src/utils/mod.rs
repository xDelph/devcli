//! General utility modules
//!
//! - `app`: Application-related utilities (e.g., environment listing)
//! - `command`: Command execution helpers
//! - `path`: Re-exports path helpers from `config-manager`
//! - `colors` / `date`: display helpers

pub mod app;
pub mod colors;
pub mod command;
pub mod path;

pub mod date;

#[cfg(test)]
mod command_tests;

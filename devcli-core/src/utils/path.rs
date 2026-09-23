//! Path expansion helpers — re-exported from `config-manager`.
//!
//! Prefer `config_manager::utils::{expand_path, contract_tilde, …}` in new code.
//! This module keeps existing `crate::utils::path::*` call sites working.

pub use config_manager::utils::{
    contract_tilde, expand_all, expand_env_vars, expand_path, expand_tilde,
};

//! App Type Detection Strategies
//!
//! These strategies identify WHAT the application is:
//! - Languages (Rust, Node.js, Python)
//! - Monorepo tools (Nx)
//! - Services (Redis, Traefik)
//! - Environment files (.env, .env.*, etc.)
//! - Frameworks (future: React, Vue, Django, etc.)
//!
//! App type strategies run first (Phase 1) with priorities 0-399.

pub mod env_files;
pub mod nodejs;
pub mod nx;
pub mod python;
pub mod redis;
pub mod rust;
pub mod traefik;

// Re-export all app type strategies
pub use env_files::EnvFilesStrategy;
pub use nodejs::NodeJsStrategy;
pub use nx::NxStrategy;
pub use python::PythonStrategy;
pub use redis::RedisStrategy;
pub use rust::RustStrategy;
pub use traefik::TraefikStrategy;

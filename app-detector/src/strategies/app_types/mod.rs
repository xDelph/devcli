//! App Type Detection Strategies
//!
//! These strategies identify WHAT the application is:
//! - Languages (Rust, Node.js, Python)
//! - Monorepo tools (Nx)
//! - Services (Redis, Traefik)
//! - Frameworks (future: React, Vue, Django, etc.)
//!
//! App type strategies run first (Phase 1) with priorities 0-399.
//!
//! Note: `.env` file discovery is owned by the `env-flow` crate
//! (discovery + loading + layering); it is intentionally not a strategy here.

pub mod nodejs;
pub mod nx;
pub mod python;
pub mod redis;
pub mod rust;
pub mod traefik;

// Re-export all app type strategies
pub use nodejs::NodeJsStrategy;
pub use nx::NxStrategy;
pub use python::PythonStrategy;
pub use redis::RedisStrategy;
pub use rust::RustStrategy;
pub use traefik::TraefikStrategy;

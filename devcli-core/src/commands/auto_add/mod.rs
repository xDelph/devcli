// Auto-add command module - Automatically detect and configure apps
// 
// This module is organized by functionality to improve maintainability:
// - single_app.rs: Handles detection and addition of single applications
// - nx_monorepo.rs: Handles Nx monorepo detection and multi-app selection
// - interactive.rs: User interaction prompts and selection interfaces
// - validation.rs: App name validation and configuration validation

// Re-export the main command entry point
pub use single_app::auto_add_command;

// Re-export functions used by tests
pub use single_app::discover_all_apps;
pub use validation::validate_app_name;

// Internal modules organized by functionality
mod single_app;
mod nx_monorepo;
mod interactive;
mod validation;
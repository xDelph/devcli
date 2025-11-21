//! Config management commands module
//! 
//! This module provides comprehensive configuration management functionality for rustycli.
//! It is organized into focused sub-modules for better maintainability:
//!
//! - `init`: Configuration file initialization and template creation
//! - `validate`: Configuration validation and dependency checking  
//! - `list`: Listing and displaying configuration information
//! - `edit`: Interactive configuration editing and command management
//! - `prompts`: User interaction helpers and input prompts
//!
//! All public functions maintain the same API as the original monolithic implementation
//! to ensure backward compatibility with existing code.

mod init;
mod validate;
mod list;
mod edit;
mod prompts;

// Re-export all public functions to maintain API compatibility
pub use init::config_init;
pub use validate::config_validate;
pub use list::{config_list, config_show, config_list_commands};
pub use edit::{
    config_edit, 
    config_add_command, 
    config_remove_command, 
    config_set_default, 
    config_edit_command,
    config_set_stage
};

// Internal helper functions are not re-exported - they remain private to this module
pub(crate) use prompts::{
    prompt_for_app,
    prompt_for_environment,
    prompt_for_command,
    prompt_for_text
};
//! Configuration validation functionality
//!
//! Provides `devcli config validate`. Rules live in
//! `config_manager_support::DevCliConfigValidator`.

use crate::config::{list_all_apps, load_config};
use crate::config_manager_support::DevCliConfigValidator;
use crate::Result;
use config_manager::validation::Validator;

/// Validate the config file
///
/// Example: `devcli config validate`
///
/// Checks paths, default commands, dependencies, cycles, and duplicate names.
/// Warnings are printed but do not fail the command.
pub async fn config_validate() -> Result<()> {
    let config = load_config()?;

    println!("Validating configuration...\n");

    let result = DevCliConfigValidator::new().validate(&config);
    let all_apps = list_all_apps(&config);

    if !result.warnings().is_empty() {
        println!("⚠ Warnings:");
        for warning in result.warnings() {
            println!("  - [{field}] {msg}", field = warning.field(), msg = warning.message());
        }
        println!();
    }

    if !result.is_valid() {
        println!("✗ Errors:");
        for error in result.errors() {
            println!("  - [{field}] {msg}", field = error.field(), msg = error.message());
        }
        println!("\nValidation failed with {} error(s)", result.error_count());
        anyhow::bail!("Config validation failed");
    }

    println!("✓ Configuration is valid!");
    println!("  - {} projects", config.projects.len());
    println!("  - {} apps", all_apps.len());

    Ok(())
}

#[cfg(test)]
#[path = "validate_test.rs"]
mod tests;

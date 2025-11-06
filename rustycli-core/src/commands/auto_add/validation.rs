// App name validation functionality
// Validates app names according to rustycli naming rules
// Ensures app names are safe for use in configuration and file systems

use crate::Result;

// Validate app name according to rules
// Returns Ok(()) if valid, Err with message if invalid
pub fn validate_app_name(name: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("App name cannot be empty. Please try again.");
    }
    
    if name.contains(' ') {
        anyhow::bail!("App name cannot contain spaces. Use dashes or underscores instead.");
    }
    
    // Additional validation: check for other problematic characters
    if name.contains('/') || name.contains('\\') {
        anyhow::bail!("App name cannot contain path separators.");
    }
    
    Ok(())
}

#[cfg(test)]
#[path = "validation_test.rs"]
mod validation_test;
// UI command implementation
// Launches the interactive Terminal User Interface (TUI)
// Provides a visual way to manage applications, view logs, and execute commands

use crate::tui::TuiApp;
use anyhow::Result;

/// Launches the interactive TUI
/// This command starts the full-screen terminal interface for managing applications
/// 
/// # Returns
/// 
/// * `Ok(())` if the TUI exits normally
/// * `Err` if there's an error initializing or running the TUI
/// 
/// # Examples
/// 
/// ```no_run
/// use rustycli_core::commands::ui_command;
/// 
/// // Launch the TUI
/// ui_command().expect("Failed to run TUI");
/// ```
pub fn ui_command() -> Result<()> {
    // Create and run the TUI application
    let mut app = TuiApp::new()?;
    app.run()?;
    
    Ok(())
}

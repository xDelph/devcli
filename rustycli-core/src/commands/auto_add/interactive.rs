// Interactive user prompts and selection interfaces
// Handles all user interaction for the auto-add command
// Uses inquire for proper arrow key navigation and selection

use crate::Result;
use inquire::{MultiSelect, Select, Confirm, Text};

use super::validation::validate_app_name;

// Interactive prompt for selecting an existing project or creating a new one
// Uses inquire for proper arrow key navigation
pub fn prompt_project_selection(config: &crate::config::Config) -> Result<String> {
    // Get all project names and sort them alphabetically, but put "global" first
    let mut projects: Vec<String> = config.projects.keys().cloned().collect();
    projects.sort();
    
    // Move "global" to the front if it exists, or add it as option 1
    let mut display_projects = Vec::new();
    let has_global = projects.contains(&"global".to_string());
    
    if has_global {
        display_projects.push("global (default)".to_string());
        // Add other projects except global
        for project in projects {
            if project != "global" {
                display_projects.push(project);
            }
        }
    } else {
        // Add "global" as first option even if it doesn't exist yet
        display_projects.push("global (default)".to_string());
        display_projects.extend(projects);
    }
    
    // Add "Create new project" option
    display_projects.push("Create new project".to_string());

    println!();

    // Use inquire's Select for arrow key navigation
    let selection = Select::new("Which project?", display_projects.clone())
        .with_help_message("Use ↑/↓ to navigate, Enter to select")
        .prompt()?;
    
    // Handle the selection
    if selection == "Create new project" {
        // Prompt for new project name
        let project_name = Text::new("New project name:")
            .with_validator(|input: &str| {
                if input.trim().is_empty() {
                    Ok(inquire::validator::Validation::Invalid("Project name cannot be empty".into()))
                } else {
                    Ok(inquire::validator::Validation::Valid)
                }
            })
            .prompt()?;
        
        Ok(project_name.trim().to_string())
    } else if selection.starts_with("global") {
        Ok("global".to_string())
    } else {
        Ok(selection)
    }
}

// Prompt to confirm or edit the detected app name using inquire
pub fn prompt_app_name(suggested_name: &str) -> Result<String> {
    // Validate the suggested name first
    if suggested_name.is_empty() {
        return prompt_custom_app_name();
    }
    
    // Use inquire's Confirm with default to true
    let use_suggested = Confirm::new(&format!("Use app name '{}'?", suggested_name))
        .with_default(true)
        .with_help_message("Press Enter to accept, or 'n' to enter a custom name")
        .prompt()?;
    
    if use_suggested {
        Ok(suggested_name.to_string())
    } else {
        prompt_custom_app_name()
    }
}

// Helper function to prompt for a custom app name with validation using inquire
fn prompt_custom_app_name() -> Result<String> {
    let app_name = Text::new("Enter app name:")
        .with_validator(|input: &str| {
            match validate_app_name(input) {
                Ok(_) => Ok(inquire::validator::Validation::Valid),
                Err(e) => Ok(inquire::validator::Validation::Invalid(e.to_string().into())),
            }
        })
        .prompt()?;
    
    Ok(app_name)
}

// Display a preview of what will be added to the config
// Shows the project/app path, type, commands, and defaults
pub fn show_preview(project: &str, app_name: &str, detected: &crate::detection::DetectedApp) {
    println!("\nPreview:");
    println!("[{}/{}]", project, app_name);
    println!("  type: {}", detected.app_type);
    println!("  path: {}", detected.path);
    
    // Show local commands and default if available
    if let Some(local_cmds) = &detected.local_commands {
        println!("  local commands: {}", local_cmds.keys().cloned().collect::<Vec<_>>().join(", "));
        if let Some(default) = &detected.suggested_local_default {
            println!("    default: {}", default);
        }
    }
    
    // Show docker commands and default if available
    if let Some(docker_cmds) = &detected.docker_commands {
        println!("  docker commands: {}", docker_cmds.keys().cloned().collect::<Vec<_>>().join(", "));
        if let Some(default) = &detected.suggested_docker_default {
            println!("    default: {}", default);
        }
    }
    
    // Show kubernetes commands if available
    if let Some(k8s_cmds) = &detected.k8s_commands {
        println!("  k8s commands: {}", k8s_cmds.keys().cloned().collect::<Vec<_>>().join(", "));
    }
    
    println!();
}

// Yes/no confirmation prompt with default to yes using inquire
pub fn confirm_default_yes(prompt: &str) -> Result<bool> {
    let confirmed = Confirm::new(prompt)
        .with_default(true)
        .with_help_message("Press Enter to confirm, or 'n' to skip")
        .prompt()?;
    
    Ok(confirmed)
}

// Interactive Nx app selection using inquire for proper UI with arrow keys
pub fn interactive_nx_app_selection(apps: &[crate::detection::DetectedApp]) -> Result<Vec<usize>> {
    // Create display options for each app with command counts
    let options: Vec<String> = apps
        .iter()
        .enumerate()
        .map(|(idx, app)| {
            let cmd_count = app.local_commands.as_ref().map(|c| c.len()).unwrap_or(0);
            format!("{}. {} ({} commands)", idx + 1, app.app_name, cmd_count)
        })
        .collect();
    
    // Use inquire's MultiSelect for checkbox-style selection
    let selected = MultiSelect::new("Select apps to add:", options.clone())
        .with_help_message("Use ↑/↓ to navigate, Space to select/deselect, Enter to confirm")
        .prompt();
    
    match selected {
        Ok(selections) => {
            // Convert selected display strings back to indices
            let mut indices = Vec::new();
            for selection in selections {
                if let Some(index) = options.iter().position(|opt| opt == &selection) {
                    indices.push(index);
                }
            }
            Ok(indices)
        }
        Err(_) => {
            // User cancelled (Ctrl+C or ESC)
            Ok(vec![])
        }
    }
}

// Interactive app selection using inquire for proper UI with arrow keys
pub fn interactive_app_selection(apps: &[crate::detection::DetectedApp]) -> Result<Vec<usize>> {
    // Create display options for each app
    let options: Vec<String> = apps
        .iter()
        .map(|app| {
            let cmd_count = app.local_commands.as_ref().map(|c| c.len()).unwrap_or(0)
                + app.docker_commands.as_ref().map(|c| c.len()).unwrap_or(0)
                + app.k8s_commands.as_ref().map(|c| c.len()).unwrap_or(0);
            
            format!("{} ({}) - {} commands | {}", 
                app.app_name, 
                app.app_type, 
                cmd_count,
                app.path
            )
        })
        .collect();

    println!();
    
    // Use inquire's MultiSelect for checkbox-style selection with arrow keys
    let selected = MultiSelect::new("Select apps to add:", options.clone())
        .with_help_message("Use ↑/↓ to navigate, Space to select/deselect, Enter to confirm")
        .prompt();
    
    match selected {
        Ok(selections) => {
            // Convert selected display strings back to indices
            let mut indices = Vec::new();
            for selection in selections {
                if let Some(index) = options.iter().position(|opt| opt == &selection) {
                    indices.push(index);
                }
            }
            Ok(indices)
        }
        Err(_) => {
            // User cancelled (Ctrl+C or ESC)
            Ok(vec![])
        }
    }
}
#[cfg(
test)]
#[path = "interactive_test.rs"]
mod interactive_test;
// Interactive user prompts and selection interfaces
// Handles all user interaction for the auto-add command
// Uses inquire for proper arrow key navigation and selection

use crate::Result;
use inquire::{MultiSelect, Select, Confirm, Text};
use std::path::Path;

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
    
    // Show orbstack commands and default if available
    if let Some(orbstack_cmds) = &detected.orbstack_commands {
        println!("  orbstack commands: {}", orbstack_cmds.keys().cloned().collect::<Vec<_>>().join(", "));
        if let Some(default) = &detected.suggested_orbstack_default {
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

/// Information about a detected environment file
#[derive(Debug, Clone)]
pub struct EnvFileInfo {
    /// Relative path from app root (e.g., ".env.qa", "config/.env.prod")
    pub relative_path: String,
    /// Detected stage name (e.g., "qa", "prod", "staging")
    pub stage_name: String,
    /// Display name for user selection (e.g., ".env.qa", "config/.env.prod")
    pub display_name: String,
}

/// Detect available environment files in an app directory and its subfolders
/// Scans for .env.* files up to 3 levels deep
/// Returns a list of EnvFileInfo with paths and detected stage names
///
/// # Arguments
/// * `app_path` - The root directory of the app to scan
///
/// # Returns
/// Vector of EnvFileInfo for all detected environment files
pub fn detect_stage_files(app_path: &Path) -> Vec<EnvFileInfo> {
    use std::fs;
    
    let mut env_files = Vec::new();

    // Helper function to scan a directory for .env.* files
    fn scan_dir(dir: &Path, app_root: &Path, current_depth: usize, max_depth: usize, results: &mut Vec<EnvFileInfo>) {
        if current_depth > max_depth {
            return;
        }

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                
                // Skip hidden directories (except .env files themselves)
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with('.') {
                            continue;
                        }
                    }
                    // Recursively scan subdirectories
                    scan_dir(&path, app_root, current_depth + 1, max_depth, results);
                } else if path.is_file() {
                    // Check if it's an .env.* file
                    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                        if filename.starts_with(".env.") && filename.len() > 5 {
                            // Extract stage name (everything after .env.)
                            let stage_name = filename[5..].to_string();
                            
                            // Calculate relative path from app root
                            let relative_path = path.strip_prefix(app_root)
                                .unwrap_or(&path)
                                .to_string_lossy()
                                .to_string();
                            
                            results.push(EnvFileInfo {
                                relative_path: relative_path.clone(),
                                stage_name,
                                display_name: relative_path,
                            });
                        }
                    }
                }
            }
        }
    }

    // Scan app directory and up to 3 levels of subdirectories
    scan_dir(app_path, app_path, 0, 3, &mut env_files);
    
    // Sort by path for consistent ordering
    env_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    env_files
}

/// Prompt user to select an environment file from detected files
/// Shows which env files were detected and allows user to choose one or skip
///
/// # Arguments
/// * `available_files` - List of detected environment files with paths and stage names
///
/// # Returns
/// Optional tuple of (stage_name, env_file_path) selected by user, or None if user chose to skip
pub fn prompt_stage_selection(available_files: &[EnvFileInfo]) -> Result<Option<(String, String)>> {
    if available_files.is_empty() {
        return Ok(None);
    }

    println!("\nDetected environment files:");
    for file in available_files {
        println!("  {} (stage: {})", file.display_name, file.stage_name);
    }

    // Build options list with available files plus "none" option
    let options: Vec<String> = available_files
        .iter()
        .map(|f| format!("{} ({})", f.display_name, f.stage_name))
        .chain(std::iter::once("none (skip stage configuration)".to_string()))
        .collect();

    let selected = Select::new("Select environment file to use:", options.clone())
        .with_help_message("Choose which env file to use by default, or 'none' to skip")
        .prompt()?;

    if selected.starts_with("none") {
        Ok(None)
    } else {
        // Find the selected file info by matching the display string
        let selected_idx = options.iter().position(|o| o == &selected).unwrap();
        let file_info = &available_files[selected_idx];
        Ok(Some((file_info.stage_name.clone(), file_info.relative_path.clone())))
    }
}

#[cfg(
test)]
#[path = "interactive_test.rs"]
mod interactive_test;
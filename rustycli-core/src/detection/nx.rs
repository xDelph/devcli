// Nx monorepo detection and handling
// Specialized logic for detecting and working with Nx monorepos

use crate::utils::path::contract_tilde;
use crate::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::detection::DetectedApp;
use crate::detection::environments::{detect_docker_commands, detect_k8s_commands};

/// Detect all apps within an Nx monorepo
/// Scans apps/ and packages/ directories for individual Nx projects
/// 
/// # Arguments
/// * `workspace_root` - Path to the Nx workspace root (where nx.json lives)
/// 
/// # Returns
/// Vector of detection results, one per app found
pub fn detect_nx_apps(workspace_root: &Path) -> Result<Vec<DetectedApp>> {
    let mut detected_apps = Vec::new();
    
    // Scan both "apps" and "packages" directories (standard Nx structure)
    for dir_name in &["apps", "packages"] {
        let dir_path = workspace_root.join(dir_name);
        
        // Skip if this directory doesn't exist in the monorepo
        if !dir_path.exists() {
            continue;
        }
        
        // Iterate through all subdirectories (each is potentially an Nx app)
        if let Ok(entries) = fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    // Try to detect this as an Nx app
                    // Some directories might not be valid apps, so we ignore errors
                    if let Ok(app) = detect_single_nx_app(&entry.path(), workspace_root) {
                        detected_apps.push(app);
                    }
                }
            }
        }
    }
    
    // Add workspace-level app with workspace commands
    if let Ok(workspace_app) = detect_nx_workspace(workspace_root) {
        detected_apps.push(workspace_app);
    }
    
    // If we found nothing, that's an error - monorepo should have apps
    if detected_apps.is_empty() {
        anyhow::bail!("No apps found in apps/ or packages/ directories");
    }
    
    Ok(detected_apps)
}

/// Detect a single Nx app within a monorepo
/// Uses "nx show project" to get available targets/commands
/// 
/// # Arguments
/// * `app_path` - Path to the specific app directory
/// * `workspace_root` - Path to the Nx workspace root (for running nx commands)
/// 
/// # Returns
/// Detection results for this specific Nx app
pub fn detect_single_nx_app(app_path: &Path, workspace_root: &Path) -> Result<DetectedApp> {
    // Extract app name from the directory name (e.g., "apps/api-backend" -> "api-backend")
    let app_name = app_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Could not extract app name"))?
        .to_string();
    
    let mut local_commands = HashMap::new();
    
    // Try to get Nx targets using "nx show project" command
    // We prefer "npx nx" over global "nx" to avoid relying on global installations
    for nx_cmd in &["npx nx", "nx"] {
        // Build the command: either "npx nx" or just "nx"
        let mut cmd = std::process::Command::new(if nx_cmd.contains("npx") { "npx" } else { "nx" });
        if nx_cmd.contains("npx") {
            cmd.arg("nx");
        }
        
        // Run "nx show project <app-name> --json" to get available targets
        // This returns a JSON object with all the app's configuration
        if let Ok(output) = cmd
            .args(["show", "project", &app_name, "--json"])
            .current_dir(workspace_root)
            .output() 
        {
            if output.status.success() {
                if let Ok(json_str) = String::from_utf8(output.stdout) {
                    // Parse the JSON response
                    if let Ok(project_config) = serde_json::from_str::<Value>(&json_str) {
                        // Extract targets (build, serve, test, lint, etc.)
                        if let Some(targets) = project_config.get("targets").and_then(|t| t.as_object()) {
                            // For each target, create a command like "npx nx run app-name:target"
                            for (target_name, _) in targets {
                                local_commands.insert(
                                    target_name.clone(),
                                    format!("{} run {}:{}", nx_cmd, app_name, target_name),
                                );
                            }
                            // Stop trying other commands once we successfully got targets
                            break;
                        }
                    }
                }
            }
        }
    }
    
    // Fallback: If nx commands failed, try reading package.json scripts
    if local_commands.is_empty() {
        // Read package.json from the app's directory
        if let Ok(content) = fs::read_to_string(app_path.join("package.json")) {
            if let Ok(json) = serde_json::from_str::<Value>(&content) {
                // Extract npm scripts and convert to "npm run" commands
                if let Some(scripts) = json.get("scripts").and_then(|v| v.as_object()) {
                    for (key, _) in scripts {
                        local_commands.insert(key.clone(), format!("npm run {}", key));
                    }
                }
            }
        }
    }
    
    // Detect Docker and Kubernetes configurations (may or may not exist)
    let docker_commands = detect_docker_commands(app_path, "nx").ok().flatten();
    let k8s_commands = detect_k8s_commands(app_path).ok().flatten();
    
    // Suggest a sensible default based on common command names
    // Prefer "serve" for dev servers, then "start", then any available command
    let suggested_local_default = if local_commands.contains_key("serve") {
        Some("serve".to_string())
    } else if local_commands.contains_key("start") {
        Some("start".to_string())
    } else {
        local_commands.keys().next().cloned()
    };
    
    // For Docker, prefer "run" as the default
    let suggested_docker_default = docker_commands.as_ref().and_then(|cmds| {
        if cmds.contains_key("run") {
            Some("run".to_string())
        } else {
            cmds.keys().next().cloned()
        }
    });
    
    // Build and return the detection result for this Nx app
    Ok(DetectedApp {
        app_type: "nx".to_string(),
        app_name,
        path: contract_tilde(app_path),
        local_commands: if local_commands.is_empty() { None } else { Some(local_commands) },
        docker_commands,
        k8s_commands,
        suggested_local_default,
        suggested_docker_default,
    })
}

/// Detect workspace-level Nx commands
/// Creates a virtual "workspace" app with workspace-level commands like run-many, affected, etc.
/// 
/// # Arguments
/// * `workspace_root` - Path to the Nx workspace root (where nx.json lives)
/// 
/// # Returns
/// Detection results for the workspace-level commands
pub fn detect_nx_workspace(workspace_root: &Path) -> Result<DetectedApp> {
    let mut local_commands = HashMap::new();
    
    // Determine which nx command to use (prefer npx nx)
    let nx_cmd = "npx nx";
    
    // Comprehensive workspace-level Nx commands based on common patterns
    // These are practical commands that work across most Nx workspaces
    
    // Basic run-many commands for all projects
    local_commands.insert("build".to_string(), format!("{} run-many --target=build --all", nx_cmd));
    local_commands.insert("test".to_string(), format!("{} run-many --target=test --all", nx_cmd));
    local_commands.insert("lint".to_string(), format!("{} run-many --target=lint --all", nx_cmd));
    local_commands.insert("start".to_string(), format!("{} run-many --target=serve --all", nx_cmd));
    
    // Parallel execution variants
    local_commands.insert("build:parallel".to_string(), format!("{} run-many --target=build --all --parallel", nx_cmd));
    local_commands.insert("test:parallel".to_string(), format!("{} run-many --target=test --all --parallel", nx_cmd));
    local_commands.insert("lint:parallel".to_string(), format!("{} run-many --target=lint --all --parallel", nx_cmd));
    local_commands.insert("start:parallel".to_string(), format!("{} run-many --target=serve --all --parallel", nx_cmd));
    
    // Project type specific commands (apps vs libs)
    local_commands.insert("build:apps".to_string(), format!("{} run-many --target=build --projects=type:application --all", nx_cmd));
    local_commands.insert("build:libs".to_string(), format!("{} run-many --target=build --projects=type:library --all", nx_cmd));
    local_commands.insert("test:apps".to_string(), format!("{} run-many --target=test --projects=type:application --all", nx_cmd));
    local_commands.insert("test:libs".to_string(), format!("{} run-many --target=test --projects=type:library --all", nx_cmd));
    local_commands.insert("lint:apps".to_string(), format!("{} run-many --target=lint --projects=type:application --all", nx_cmd));
    local_commands.insert("lint:libs".to_string(), format!("{} run-many --target=lint --projects=type:library --all", nx_cmd));
    local_commands.insert("start:apps".to_string(), format!("{} run-many --target=serve --projects=type:application --all", nx_cmd));
    
    // Configuration-specific commands
    local_commands.insert("build:prod".to_string(), format!("{} run-many --target=build --configuration=production --all", nx_cmd));
    local_commands.insert("start:prod".to_string(), format!("{} run-many --target=serve --configuration=production --all", nx_cmd));
    local_commands.insert("start:dev".to_string(), format!("{} run-many --target=serve --configuration=development --all", nx_cmd));
    
    // Affected commands (only run what changed)
    local_commands.insert("affected".to_string(), format!("{} affected", nx_cmd));
    local_commands.insert("affected:build".to_string(), format!("{} affected --target=build", nx_cmd));
    local_commands.insert("affected:test".to_string(), format!("{} affected --target=test", nx_cmd));
    local_commands.insert("affected:lint".to_string(), format!("{} affected --target=lint", nx_cmd));
    local_commands.insert("affected:e2e".to_string(), format!("{} affected --target=e2e", nx_cmd));
    
    // Utility commands
    local_commands.insert("graph".to_string(), format!("{} graph", nx_cmd));
    local_commands.insert("list-projects".to_string(), format!("{} show projects", nx_cmd));
    local_commands.insert("list".to_string(), format!("{} list", nx_cmd));
    
    // Workspace management commands
    local_commands.insert("reset".to_string(), format!("{} reset", nx_cmd));
    local_commands.insert("repair".to_string(), format!("{} repair", nx_cmd));
    local_commands.insert("migrate".to_string(), format!("{} migrate", nx_cmd));
    local_commands.insert("daemon".to_string(), format!("{} daemon", nx_cmd));
    
    // Detect Docker and Kubernetes configurations at workspace level
    let docker_commands = detect_docker_commands(workspace_root, "nx").ok().flatten();
    let k8s_commands = detect_k8s_commands(workspace_root).ok().flatten();
    
    // Suggest "build" as default for workspace (most commonly used)
    let suggested_local_default = if local_commands.contains_key("build") {
        Some("build".to_string())
    } else if local_commands.contains_key("graph") {
        Some("graph".to_string())
    } else {
        local_commands.keys().next().cloned()
    };
    
    // For Docker, prefer "run" as the default
    let suggested_docker_default = docker_commands.as_ref().and_then(|cmds| {
        if cmds.contains_key("run") {
            Some("run".to_string())
        } else {
            cmds.keys().next().cloned()
        }
    });
    
    // Build and return the detection result for the workspace
    Ok(DetectedApp {
        app_type: "nx-workspace".to_string(),
        app_name: "workspace".to_string(),
        path: contract_tilde(workspace_root),
        local_commands: if local_commands.is_empty() { None } else { Some(local_commands) },
        docker_commands,
        k8s_commands,
        suggested_local_default,
        suggested_docker_default,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_nx_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let nx_json = temp_dir.path().join("nx.json");
        fs::write(&nx_json, r#"{"version": 2}"#).unwrap();

        let result = detect_nx_workspace(temp_dir.path()).unwrap();
        
        assert_eq!(result.app_type, "nx-workspace");
        assert_eq!(result.app_name, "workspace");
        assert!(result.local_commands.is_some());
        
        let commands = result.local_commands.unwrap();
        assert!(commands.contains_key("build"));
        assert!(commands.contains_key("test"));
        assert!(commands.contains_key("lint"));
        assert!(commands.contains_key("start"));
        assert!(commands.contains_key("build:parallel"));
        assert!(commands.contains_key("build:apps"));
        assert!(commands.contains_key("build:libs"));
        assert!(commands.contains_key("affected:build"));
        assert!(commands.contains_key("graph"));
        
        // Should suggest "build" as default
        assert_eq!(result.suggested_local_default, Some("build".to_string()));
        
        // Check some specific command formats
        assert_eq!(commands.get("build"), Some(&"npx nx run-many --target=build --all".to_string()));
        assert_eq!(commands.get("build:parallel"), Some(&"npx nx run-many --target=build --all --parallel".to_string()));
        assert_eq!(commands.get("build:apps"), Some(&"npx nx run-many --target=build --projects=type:application --all".to_string()));
        assert_eq!(commands.get("affected:build"), Some(&"npx nx affected --target=build".to_string()));
    }

    #[test]
    fn test_detect_nx_apps_includes_workspace() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create nx.json
        let nx_json = temp_dir.path().join("nx.json");
        fs::write(&nx_json, r#"{"version": 2}"#).unwrap();
        
        // Create apps directory with one app
        let apps_dir = temp_dir.path().join("apps");
        fs::create_dir(&apps_dir).unwrap();
        let app_dir = apps_dir.join("test-app");
        fs::create_dir(&app_dir).unwrap();
        let package_json = app_dir.join("package.json");
        fs::write(&package_json, r#"{"name": "test-app", "scripts": {"build": "nx build"}}"#).unwrap();

        let result = detect_nx_apps(temp_dir.path()).unwrap();
        
        // Should have both the app and the workspace
        assert_eq!(result.len(), 2);
        
        // Find workspace app
        let workspace_app = result.iter().find(|app| app.app_name == "workspace");
        assert!(workspace_app.is_some());
        
        let workspace_app = workspace_app.unwrap();
        assert_eq!(workspace_app.app_type, "nx-workspace");
        assert!(workspace_app.local_commands.is_some());
    }
}
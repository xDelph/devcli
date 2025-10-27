// App detection module - Automatically discovers app configurations
// Scans directories to identify app types (Node.js, Nx, Python, Redis, Traefik)
// and detect available environments (local, Docker, Kubernetes)
//
// Detection is two-phase:
// 1. App Type Detection: What is the app? (nodejs, nx, python, etc.)
// 2. Environment Detection: How can we run it? (local commands, docker, k8s)

use crate::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// Structure holding all detection results for an app
// Contains app metadata, commands for different environments, and suggested defaults
#[derive(Debug, Clone)]
pub struct DetectedApp {
    pub app_type: String,               // Type of app: nodejs, nx, python, redis, traefik
    pub app_name: String,               // Name extracted from config files or directory
    pub path: String,                   // Absolute path to app directory
    pub local_commands: Option<HashMap<String, String>>,  // Commands for local environment
    pub docker_commands: Option<HashMap<String, String>>, // Commands for Docker environment
    pub k8s_commands: Option<HashMap<String, String>>,    // Commands for Kubernetes
    pub suggested_local_default: Option<String>,          // Suggested default local command
    pub suggested_docker_default: Option<String>,         // Suggested default docker command
}

// Main detection function for a single app
// Runs two-phase detection: type first, then environments
// Args:
//   - path: Directory to scan for app configuration
// Returns: Complete detection results with all environments
pub fn detect_app(path: &Path) -> Result<DetectedApp> {
    // Phase 1: Detect what type of app this is
    let app_type = detect_app_type(path)?;
    
    // Extract app name from configuration files or directory name
    let app_name = extract_app_name(path, &app_type)?;
    
    // Phase 2: Detect how we can run this app (local, docker, k8s)
    let local_commands = detect_local_commands(path, &app_type)?;
    let docker_commands = detect_docker_commands(path, &app_type)?;
    let k8s_commands = detect_k8s_commands(path)?;
    
    // Suggest sensible defaults based on common command names
    let suggested_local_default = suggest_local_default(&app_type, &local_commands);
    let suggested_docker_default = suggest_docker_default(&docker_commands);
    
    // Build and return the complete detection result
    Ok(DetectedApp {
        app_type,
        app_name,
        path: path.to_string_lossy().to_string(),
        local_commands,
        docker_commands,
        k8s_commands,
        suggested_local_default,
        suggested_docker_default,
    })
}

// Detect all apps within an Nx monorepo
// Scans apps/ and packages/ directories for individual Nx projects
// Args:
//   - workspace_root: Path to the Nx workspace root (where nx.json lives)
// Returns: Vector of detection results, one per app found
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
    
    // If we found nothing, that's an error - monorepo should have apps
    if detected_apps.is_empty() {
        anyhow::bail!("No apps found in apps/ or packages/ directories");
    }
    
    Ok(detected_apps)
}

// Detect a single Nx app within a monorepo
// Uses "nx show project" to get available targets/commands
// Args:
//   - app_path: Path to the specific app directory
//   - workspace_root: Path to the Nx workspace root (for running nx commands)
// Returns: Detection results for this specific Nx app
fn detect_single_nx_app(app_path: &Path, workspace_root: &Path) -> Result<DetectedApp> {
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
                            // For each target, create a command like "npx nx build app-name"
                            for (target_name, _) in targets {
                                local_commands.insert(
                                    target_name.clone(),
                                    format!("{} {} {}", nx_cmd, target_name, app_name),
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
        path: app_path.to_string_lossy().to_string(),
        local_commands: if local_commands.is_empty() { None } else { Some(local_commands) },
        docker_commands,
        k8s_commands,
        suggested_local_default,
        suggested_docker_default,
    })
}

// Phase 1: Detect what type of app this is
// Checks for marker files that identify different app types
// Priority: nx > nodejs > redis > traefik > python
// Args:
//   - path: Directory to check for app type markers
// Returns: App type as string (nx, nodejs, python, redis, traefik)
fn detect_app_type(path: &Path) -> Result<String> {
    // Check for Nx monorepo (highest priority)
    // nx.json is the marker file for Nx workspaces
    if path.join("nx.json").exists() {
        return Ok("nx".to_string());
    }
    
    // Check for Node.js app (package.json)
    if path.join("package.json").exists() {
        return Ok("nodejs".to_string());
    }
    
    // Check for Redis (redis.conf file)
    if path.join("redis.conf").exists() {
        return Ok("redis".to_string());
    }
    
    // Check for Traefik or Docker Compose configurations
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            
            // Traefik configuration files
            if file_name_str == "traefik.yml" 
                || file_name_str == "traefik.yaml" 
                || file_name_str == "traefik.toml" {
                return Ok("traefik".to_string());
            }
            
            // Check docker-compose for redis or traefik services
            if file_name_str == "docker-compose.yml" || file_name_str == "docker-compose.yaml" {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if content.contains("redis:") {
                        return Ok("redis".to_string());
                    }
                    if content.contains("traefik") {
                        return Ok("traefik".to_string());
                    }
                }
            }
        }
    }
    
    // Check for Python app (requirements.txt, pyproject.toml, or setup.py)
    if path.join("requirements.txt").exists() 
        || path.join("pyproject.toml").exists() 
        || path.join("setup.py").exists() {
        return Ok("python".to_string());
    }
    
    // No recognized app type found
    anyhow::bail!("No supported app type detected in {}", path.display())
}

// Extract the app name from configuration files or directory name
// Args:
//   - path: Directory path
//   - app_type: The detected app type
// Returns: App name as string
fn extract_app_name(path: &Path, app_type: &str) -> Result<String> {
    // For Node.js and Nx apps, check package.json for the "name" field
    if app_type == "nodejs" || app_type == "nx" {
        if let Ok(content) = fs::read_to_string(path.join("package.json")) {
            if let Ok(json) = serde_json::from_str::<Value>(&content) {
                if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                    // Handle scoped packages like "@scope/package-name"
                    // We only want the "package-name" part
                    if name.contains('/') {
                        if let Some(short_name) = name.split('/').last() {
                            return Ok(short_name.to_string());
                        }
                    }
                    return Ok(name.to_string());
                }
            }
        }
    }
    
    // For Python apps, check pyproject.toml for the project name
    if app_type == "python" {
        if let Ok(content) = fs::read_to_string(path.join("pyproject.toml")) {
            for line in content.lines() {
                if line.starts_with("name") && line.contains('=') {
                    if let Some(name) = line.split('=').nth(1) {
                        // Remove quotes and whitespace
                        let name = name.trim().trim_matches('"').trim_matches('\'');
                        return Ok(name.to_string());
                    }
                }
            }
        }
    }
    
    // Fallback: Use the directory name as the app name
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Could not extract app name from path"))
}

// Phase 2a: Detect commands for local environment
// Extracts commands based on app type (npm scripts, nx targets, etc.)
// Args:
//   - path: App directory
//   - app_type: The detected app type
// Returns: HashMap of command name -> command string, or None if no commands found
fn detect_local_commands(path: &Path, app_type: &str) -> Result<Option<HashMap<String, String>>> {
    let mut commands = HashMap::new();
    
    match app_type {
        "nodejs" => {
            if let Ok(content) = fs::read_to_string(path.join("package.json")) {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    if let Some(scripts) = json.get("scripts").and_then(|v| v.as_object()) {
                        for (key, value) in scripts {
                            if let Some(_cmd) = value.as_str() {
                                commands.insert(key.clone(), format!("npm run {}", key));
                            }
                        }
                    }
                }
            }
        }
        "nx" => {
            if let Ok(content) = fs::read_to_string(path.join("package.json")) {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    if let Some(scripts) = json.get("scripts").and_then(|v| v.as_object()) {
                        for (key, _) in scripts {
                            commands.insert(key.clone(), format!("npm run {}", key));
                        }
                    }
                }
            }
        }
        "python" => {
            commands.insert("start".to_string(), "python main.py".to_string());
            if path.join("tests").exists() || path.join("test").exists() {
                commands.insert("test".to_string(), "pytest".to_string());
            }
            if path.join("requirements.txt").exists() {
                commands.insert("install".to_string(), "pip install -r requirements.txt".to_string());
            }
        }
        "redis" => {
            commands.insert("start".to_string(), "redis-server".to_string());
            if path.join("redis.conf").exists() {
                commands.insert("start-config".to_string(), "redis-server redis.conf".to_string());
            }
        }
        "traefik" => {
            for file in &["traefik.yml", "traefik.yaml", "traefik.toml"] {
                if path.join(file).exists() {
                    commands.insert("start".to_string(), format!("traefik --configFile={}", file));
                    break;
                }
            }
        }
        _ => {}
    }
    
    if commands.is_empty() {
        Ok(None)
    } else {
        Ok(Some(commands))
    }
}

// Phase 2b: Detect Docker commands if Dockerfile exists
// Searches for Dockerfile up to 2 levels deep
// Generates build/run/stop commands based on app type
// Args:
//   - path: App directory
//   - app_type: The detected app type (affects port mappings)
// Returns: HashMap of Docker commands, or None if no Dockerfile found
fn detect_docker_commands(path: &Path, app_type: &str) -> Result<Option<HashMap<String, String>>> {
    // Search for Dockerfile (up to 2 levels deep)
    let dockerfile = find_dockerfile(path)?;
    
    // No Dockerfile = no Docker commands
    if dockerfile.is_none() {
        return Ok(None);
    }
    
    let mut commands = HashMap::new();
    
    // Use directory name as the image name
    let app_name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("app");
    
    // Always add a build command
    commands.insert("build".to_string(), format!("docker build -t {} .", app_name));
    
    // Add run command with appropriate port mappings based on app type
    match app_type {
        "nodejs" | "nx" => {
            // Node.js apps typically run on port 3000
            commands.insert(
                "run".to_string(),
                format!("docker run --name {} --rm -p 3000:3000 {}", app_name, app_name),
            );
        }
        "python" => {
            // Python web apps typically run on port 8000
            commands.insert(
                "run".to_string(),
                format!("docker run --name {} --rm -p 8000:8000 {}", app_name, app_name),
            );
        }
        "redis" => {
            // Redis default port is 6379
            commands.insert(
                "run".to_string(),
                format!("docker run --name {} --rm -p 6379:6379 {}", app_name, app_name),
            );
        }
        "traefik" => {
            // Traefik uses ports 80 (HTTP) and 443 (HTTPS)
            commands.insert(
                "run".to_string(),
                format!("docker run --name {} --rm -p 80:80 -p 443:443 {}", app_name, app_name),
            );
        }
        _ => {
            // Unknown app type - no port mapping
            commands.insert(
                "run".to_string(),
                format!("docker run --name {} --rm {}", app_name, app_name),
            );
        }
    }
    
    // Add stop command
    commands.insert("stop".to_string(), format!("docker stop {}", app_name));
    
    Ok(Some(commands))
}

// Phase 2c: Detect Kubernetes commands if manifests exist
// Searches for k8s/*.yaml files or *.k8s.yaml files up to 2 levels deep
// Args:
//   - path: App directory
// Returns: HashMap of kubectl commands, or None if no k8s files found
fn detect_k8s_commands(path: &Path) -> Result<Option<HashMap<String, String>>> {
    // Search for Kubernetes manifest files
    let k8s_files = find_k8s_files(path)?;
    
    // No manifests found = no Kubernetes commands
    if k8s_files.is_empty() {
        return Ok(None);
    }
    
    let mut commands = HashMap::new();
    
    // Determine the path to use for kubectl commands
    // If files are in k8s/ directory, use "k8s/" otherwise use "."
    let k8s_path = if k8s_files[0].starts_with("k8s/") {
        "k8s/"
    } else {
        "."
    };
    
    // Standard kubectl commands for deployment
    commands.insert("apply".to_string(), format!("kubectl apply -f {}", k8s_path));
    commands.insert("delete".to_string(), format!("kubectl delete -f {}", k8s_path));
    commands.insert("restart".to_string(), "kubectl rollout restart deployment".to_string());
    
    Ok(Some(commands))
}

// Search for Dockerfile up to 2 levels deep in the directory tree
// Args:
//   - path: Root directory to search
// Returns: Path to Dockerfile if found, None otherwise
fn find_dockerfile(path: &Path) -> Result<Option<PathBuf>> {
    // Check current directory first
    if path.join("Dockerfile").exists() {
        return Ok(Some(path.join("Dockerfile")));
    }
    
    // Check 1 level deep (subdirectories)
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let subdir_dockerfile = entry.path().join("Dockerfile");
                if subdir_dockerfile.exists() {
                    return Ok(Some(subdir_dockerfile));
                }
                
                // Check 2 levels deep (nested subdirectories)
                if let Ok(subentries) = fs::read_dir(entry.path()) {
                    for subentry in subentries.flatten() {
                        if subentry.path().is_dir() {
                            let nested_dockerfile = subentry.path().join("Dockerfile");
                            if nested_dockerfile.exists() {
                                return Ok(Some(nested_dockerfile));
                            }
                        }
                    }
                }
            }
        }
    }
    
    // No Dockerfile found
    Ok(None)
}

// Search for Kubernetes manifest files up to 2 levels deep
// Looks for:
//   - k8s/*.yaml files
//   - *.k8s.yaml files in current directory
//   - *.k8s.yaml files in subdirectories
// Args:
//   - path: Root directory to search
// Returns: List of relative paths to k8s manifest files
fn find_k8s_files(path: &Path) -> Result<Vec<String>> {
    let mut k8s_files = Vec::new();
    
    // Check for dedicated k8s/ directory (most common pattern)
    let k8s_dir = path.join("k8s");
    if k8s_dir.exists() && k8s_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&k8s_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                // Add any YAML files in the k8s directory
                if file_name_str.ends_with(".yaml") || file_name_str.ends_with(".yml") {
                    k8s_files.push(format!("k8s/{}", file_name_str));
                }
            }
        }
    }
    
    // Check current directory for *.k8s.yaml files
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            
            // Check for *.k8s.yaml pattern in current directory
            if file_name_str.ends_with(".k8s.yaml") || file_name_str.ends_with(".k8s.yml") {
                k8s_files.push(file_name_str.to_string());
            }
            
            // Check subdirectories (1 level deep) for *.k8s.yaml files
            // Skip the k8s/ directory as we already checked it above
            if entry.path().is_dir() && entry.file_name() != "k8s" {
                if let Ok(subentries) = fs::read_dir(entry.path()) {
                    for subentry in subentries.flatten() {
                        let sub_file_name = subentry.file_name();
                        let sub_file_name_str = sub_file_name.to_string_lossy();
                        // Check for *.k8s.yaml pattern in subdirectories
                        if sub_file_name_str.ends_with(".k8s.yaml") || sub_file_name_str.ends_with(".k8s.yml") {
                            k8s_files.push(format!("{}/{}", file_name_str, sub_file_name_str));
                        }
                    }
                }
            }
        }
    }
    
    Ok(k8s_files)
}

// Suggest a sensible default command for local environment
// Prefers common dev server commands like "serve", "start", "dev"
// Args:
//   - app_type: The app type
//   - commands: Available local commands
// Returns: Suggested command name, or None if no commands
fn suggest_local_default(app_type: &str, commands: &Option<HashMap<String, String>>) -> Option<String> {
    if let Some(cmds) = commands {
        // Choose default based on app type and common command names
        match app_type {
            "nodejs" | "nx" => {
                // For Node/Nx apps, prefer dev server commands
                if cmds.contains_key("serve") {
                    return Some("serve".to_string());
                }
                if cmds.contains_key("start") {
                    return Some("start".to_string());
                }
                if cmds.contains_key("dev") {
                    return Some("dev".to_string());
                }
            }
            "python" => {
                // For Python apps, "start" is most common
                if cmds.contains_key("start") {
                    return Some("start".to_string());
                }
            }
            "redis" | "traefik" => {
                // For services, "start" is the standard command
                if cmds.contains_key("start") {
                    return Some("start".to_string());
                }
            }
            _ => {}
        }
        
        // Fallback: just use the first available command
        cmds.keys().next().cloned()
    } else {
        None
    }
}

// Suggest a default command for Docker environment
// Args:
//   - commands: Available Docker commands
// Returns: Suggested command name (usually "run"), or None if no commands
fn suggest_docker_default(commands: &Option<HashMap<String, String>>) -> Option<String> {
    if let Some(cmds) = commands {
        // For Docker, "run" is almost always the default
        if cmds.contains_key("run") {
            return Some("run".to_string());
        }
        // Fallback to first available command
        cmds.keys().next().cloned()
    } else {
        None
    }
}



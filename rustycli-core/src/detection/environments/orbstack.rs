// OrbStack environment detection
// Similar to Docker but uses OrbStack context and parses .env files
// OrbStack doesn't support --env-file, so we parse and pass env vars as -e flags

use crate::detection::dockerfile;
use crate::detection::utils::find_dockerfile;
use crate::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Parse a .env file and return key-value pairs
/// Handles basic .env format: KEY=VALUE
/// Skips empty lines and comments (lines starting with #)
/// Empty values are replaced with "XXX" placeholder
pub fn parse_env_file(env_path: &Path) -> Result<HashMap<String, String>> {
    let mut env_vars = HashMap::new();
    
    if !env_path.exists() {
        return Ok(env_vars);
    }
    
    let content = fs::read_to_string(env_path)?;
    
    for line in content.lines() {
        let line = line.trim();
        
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        // Parse KEY=VALUE format
        if let Some(pos) = line.find('=') {
            let key = line[..pos].trim().to_string();
            let value = line[pos + 1..].trim().to_string();
            
            // Remove quotes if present
            let value = value.trim_matches('"').trim_matches('\'').to_string();
            
            // Replace empty values with XXX placeholder
            let value = if value.is_empty() {
                "XXX".to_string()
            } else {
                value
            };
            
            env_vars.insert(key, value);
        }
    }
    
    Ok(env_vars)
}

/// Find the .env file with priority order, supporting stage-specific files
/// 
/// Priority order when stage is specified:
/// 1. Stage-specific file at Dockerfile level (e.g., docker/.env.dev)
/// 2. Stage-specific file at root level (e.g., .env.dev)
/// 3. Base .env file at Dockerfile level
/// 4. Base .env file at root level
/// 
/// Priority order when stage is NOT specified:
/// 1. .env file at the Dockerfile level (using dockerfile_path from config if provided)
/// 2. .env file at the Dockerfile level (by searching for Dockerfile if not in config)
/// 3. .env file at the root level
/// 
/// # Arguments
/// * `app_path` - The root directory of the app
/// * `dockerfile_path` - Optional relative path to Dockerfile from config (e.g., "docker/Dockerfile")
/// * `stage` - Optional deployment stage (dev, qa, preprod, prod)
/// 
/// # Returns
/// Path to .env file if found, relative to app_path for Docker compatibility
pub fn find_env_file(
    app_path: &Path,
    dockerfile_path: Option<&str>,
    stage: Option<&str>,
) -> Result<Option<String>> {
    // Helper function to check if a file exists and return its relative path
    let check_file = |path: &std::path::PathBuf| -> Option<String> {
        if path.exists() {
            path.strip_prefix(app_path)
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        } else {
            None
        }
    };

    // Determine the Dockerfile directory
    let dockerfile_dir = if let Some(dockerfile_rel_path) = dockerfile_path {
        let dockerfile_full_path = app_path.join(dockerfile_rel_path);
        dockerfile_full_path.parent().map(|p| p.to_path_buf())
    } else {
        find_dockerfile(app_path)?
            .and_then(|df| df.parent().map(|p| p.to_path_buf()))
    };

    // If stage is specified, try stage-specific files first
    if let Some(stage_name) = stage {
        let stage_filename = format!(".env.{}", stage_name);

        // 1. Try stage-specific file at Dockerfile level
        if let Some(ref dir) = dockerfile_dir {
            if let Some(path) = check_file(&dir.join(&stage_filename)) {
                return Ok(Some(path));
            }
        }

        // 2. Try stage-specific file at root level
        if let Some(path) = check_file(&app_path.join(&stage_filename)) {
            return Ok(Some(path));
        }
    }

    // 3. Fall back to base .env at Dockerfile level
    if let Some(ref dir) = dockerfile_dir {
        if let Some(path) = check_file(&dir.join(".env")) {
            return Ok(Some(path));
        }
    }

    // 4. Fall back to base .env at root level
    if let Some(path) = check_file(&app_path.join(".env")) {
        return Ok(Some(path));
    }

    Ok(None)
}

/// Load environment variables from .env files for runtime use
/// Searches for .env files in the app directory and returns them as a HashMap
/// This is used at runtime when executing OrbStack commands
/// 
/// Priority order when stage is specified:
/// 1. Stage-specific file at Dockerfile level (e.g., docker/.env.dev)
/// 2. Stage-specific file at root level (e.g., .env.dev)
/// 3. Base .env file at Dockerfile level
/// 4. Base .env file at root level
/// 
/// Priority order when stage is NOT specified:
/// 1. .env file at the Dockerfile level (using dockerfile_path from config if provided)
/// 2. .env file at the Dockerfile level (by searching for Dockerfile if not in config)
/// 3. .env file at the root level
/// 
/// # Arguments
/// * `app_path` - The root directory of the app
/// * `dockerfile_path` - Optional relative path to Dockerfile from config (e.g., "docker/Dockerfile")
/// * `stage` - Optional deployment stage (dev, qa, preprod, prod)
/// 
/// # Returns
/// HashMap of environment variables loaded from .env files
pub fn load_env_vars_for_runtime(
    app_path: &Path,
    dockerfile_path: Option<&str>,
    stage: Option<&str>,
) -> Result<HashMap<String, String>> {
    // Use find_env_file to determine which file to load based on priority
    if let Some(env_file_path) = find_env_file(app_path, dockerfile_path, stage)? {
        let full_path = app_path.join(&env_file_path);
        return parse_env_file(&full_path);
    }
    
    // No env file found, return empty HashMap
    Ok(HashMap::new())
}

/// Phase 2c: Detect OrbStack commands if Dockerfile exists
/// Similar to Docker detection but uses OrbStack context
/// Environment variables from .env files are applied at runtime, not baked into commands
/// 
/// # Arguments
/// * `path` - App directory
/// * `app_type` - The detected app type (affects port mappings)
/// 
/// # Returns
/// HashMap of OrbStack commands, or None if no Dockerfile found
pub fn detect_orbstack_commands(path: &Path, app_type: &str) -> Result<Option<HashMap<String, String>>> {
    // Search for Dockerfile (up to 2 levels deep)
    let dockerfile = find_dockerfile(path)?;
    
    // No Dockerfile = no OrbStack commands
    if dockerfile.is_none() {
        return Ok(None);
    }
    
    let mut commands = HashMap::new();
    
    // Use directory name as the image name
    let app_name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("app");
    
    let dockerfile_path = dockerfile.as_ref().unwrap();
    
    // Check if this is a multi-stage build
    let stages = dockerfile::parse_dockerfile(dockerfile_path).unwrap_or_default();
    
    if !stages.is_empty() {
        // Multi-stage Dockerfile detected!
        // Generate commands for each stage
        for stage in &stages {
            let stage_name = &stage.name;
            
            // Build command for this specific stage
            commands.insert(
                stage_name.clone(),
                format!("docker --context orbstack build --target {} -t {}:{} .", stage_name, app_name, stage_name),
            );
            
            // For test stages, also add a run command
            // Note: env vars will be applied at runtime, not here
            if stage_name.to_lowercase().contains("test") {
                commands.insert(
                    format!("{}-run", stage_name),
                    format!("docker --context orbstack run --rm {}:{}", app_name, stage_name)
                );
            }
        }
    }
    
    // Always add a general build command (builds the final stage)
    commands.insert("build".to_string(), format!("docker --context orbstack build -t {} .", app_name));
    
    // Add run command with appropriate port mappings based on app type
    // Note: env vars from .env files will be applied at runtime via process env_vars
    let run_cmd = match app_type {
        "nodejs" | "nx" => {
            // Node.js apps typically run on port 3000
            format!("docker --context orbstack run --name {} --rm -p 3000:3000 {}", app_name, app_name)
        }
        "python" => {
            // Python web apps typically run on port 8000
            format!("docker --context orbstack run --name {} --rm -p 8000:8000 {}", app_name, app_name)
        }
        "redis" => {
            // Redis default port is 6379
            format!("docker --context orbstack run --name {} --rm -p 6379:6379 {}", app_name, app_name)
        }
        "traefik" => {
            // Traefik uses ports 80 (HTTP) and 443 (HTTPS)
            format!("docker --context orbstack run --name {} --rm -p 80:80 -p 443:443 {}", app_name, app_name)
        }
        _ => {
            // Unknown app type - no port mapping
            format!("docker --context orbstack run --name {} --rm {}", app_name, app_name)
        }
    };
    
    commands.insert("run".to_string(), run_cmd);
    
    // Add stop command
    commands.insert("stop".to_string(), format!("docker --context orbstack stop {}", app_name));
    
    Ok(Some(commands))
}

/// Suggest a default command for OrbStack environment
/// 
/// # Arguments
/// * `commands` - Available OrbStack commands
/// 
/// # Returns
/// Suggested command name (usually "run"), or None if no commands
pub fn suggest_orbstack_default(commands: &Option<HashMap<String, String>>) -> Option<String> {
    if let Some(cmds) = commands {
        // For OrbStack, "run" is almost always the default
        if cmds.contains_key("run") {
            return Some("run".to_string());
        }
        // Fallback to first available command
        cmds.keys().next().cloned()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_parse_env_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "# Comment").unwrap();
        writeln!(file, "KEY1=value1").unwrap();
        writeln!(file, "KEY2=\"value2\"").unwrap();
        writeln!(file, "KEY3='value3'").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "KEY4=value4").unwrap();
        file.flush().unwrap();
        
        let env_vars = parse_env_file(file.path()).unwrap();
        assert_eq!(env_vars.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(env_vars.get("KEY2"), Some(&"value2".to_string()));
        assert_eq!(env_vars.get("KEY3"), Some(&"value3".to_string()));
        assert_eq!(env_vars.get("KEY4"), Some(&"value4".to_string()));
    }
    
    #[test]
    fn test_parse_env_file_empty_values() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "KEY1=").unwrap();
        writeln!(file, "KEY2= ").unwrap();
        writeln!(file, "KEY3=\"\"").unwrap();
        writeln!(file, "KEY4=value").unwrap();
        file.flush().unwrap();
        
        let env_vars = parse_env_file(file.path()).unwrap();
        // Empty values should be replaced with XXX
        assert_eq!(env_vars.get("KEY1"), Some(&"XXX".to_string()));
        assert_eq!(env_vars.get("KEY2"), Some(&"XXX".to_string()));
        assert_eq!(env_vars.get("KEY3"), Some(&"XXX".to_string()));
        assert_eq!(env_vars.get("KEY4"), Some(&"value".to_string()));
    }
    
    #[test]
    fn test_load_env_vars_for_runtime() {
        use std::io::Write;
        use tempfile::TempDir;
        
        let temp_dir = TempDir::new().unwrap();
        let env_file = temp_dir.path().join(".env");
        
        let mut file = std::fs::File::create(&env_file).unwrap();
        writeln!(file, "KEY1=value1").unwrap();
        writeln!(file, "KEY2=value2").unwrap();
        file.flush().unwrap();
        
        let env_vars = load_env_vars_for_runtime(temp_dir.path(), None, None).unwrap();
        assert_eq!(env_vars.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(env_vars.get("KEY2"), Some(&"value2".to_string()));
    }
}

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

/// Find the .env file with priority order
/// 
/// Priority order:
/// 1. .env file at the Dockerfile level (using dockerfile_path from config if provided)
/// 2. .env file at the Dockerfile level (by searching for Dockerfile if not in config)
/// 3. .env file at the root level
/// 
/// # Arguments
/// * `app_path` - The root directory of the app
/// * `dockerfile_path` - Optional relative path to Dockerfile from config (e.g., "docker/Dockerfile")
/// 
/// # Returns
/// Path to .env file if found, relative to app_path for Docker compatibility
pub fn find_env_file(app_path: &Path, dockerfile_path: Option<&str>) -> Result<Option<String>> {
    // First, try to use dockerfile_path from config if provided
    if let Some(dockerfile_rel_path) = dockerfile_path {
        let dockerfile_full_path = app_path.join(dockerfile_rel_path);
        if let Some(dockerfile_dir) = dockerfile_full_path.parent() {
            let dockerfile_env = dockerfile_dir.join(".env");
            if dockerfile_env.exists() {
                // Return relative path from app_path
                if let Ok(relative) = dockerfile_env.strip_prefix(app_path) {
                    return Ok(Some(relative.to_string_lossy().to_string()));
                }
            }
        }
    }
    
    // If dockerfile_path not in config, try to find Dockerfile dynamically
    if dockerfile_path.is_none() {
        if let Ok(Some(found_dockerfile)) = find_dockerfile(app_path) {
            if let Some(dockerfile_dir) = found_dockerfile.parent() {
                let dockerfile_env = dockerfile_dir.join(".env");
                if dockerfile_env.exists() {
                    // Return relative path from app_path
                    if let Ok(relative) = dockerfile_env.strip_prefix(app_path) {
                        return Ok(Some(relative.to_string_lossy().to_string()));
                    }
                }
            }
        }
    }
    
    // Fall back to root .env file
    let root_env = app_path.join(".env");
    if root_env.exists() {
        return Ok(Some(".env".to_string()));
    }
    
    Ok(None)
}

/// Load environment variables from .env files for runtime use
/// Searches for .env files in the app directory and returns them as a HashMap
/// This is used at runtime when executing OrbStack commands
/// 
/// Priority order:
/// 1. .env file at the Dockerfile level (using dockerfile_path from config if provided)
/// 2. .env file at the Dockerfile level (by searching for Dockerfile if not in config)
/// 3. .env file at the root level
/// 
/// # Arguments
/// * `app_path` - The root directory of the app
/// * `dockerfile_path` - Optional relative path to Dockerfile from config (e.g., "docker/Dockerfile")
/// 
/// # Returns
/// HashMap of environment variables loaded from .env files
pub fn load_env_vars_for_runtime(app_path: &Path, dockerfile_path: Option<&str>) -> Result<HashMap<String, String>> {
    let mut env_vars = HashMap::new();
    
    // First, try to use dockerfile_path from config if provided
    if let Some(dockerfile_rel_path) = dockerfile_path {
        let dockerfile_full_path = app_path.join(dockerfile_rel_path);
        
        if let Some(dockerfile_dir) = dockerfile_full_path.parent() {
            let dockerfile_env = dockerfile_dir.join(".env");
            
            if dockerfile_env.exists() {
                // Prioritize .env at Dockerfile level
                env_vars.extend(parse_env_file(&dockerfile_env)?);
                return Ok(env_vars);
            }
        }
    }
    
    // If dockerfile_path not in config, try to find Dockerfile dynamically
    if dockerfile_path.is_none() {
        if let Ok(Some(found_dockerfile)) = find_dockerfile(app_path) {
            if let Some(dockerfile_dir) = found_dockerfile.parent() {
                let dockerfile_env = dockerfile_dir.join(".env");
                if dockerfile_env.exists() {
                    // Prioritize .env at Dockerfile level
                    env_vars.extend(parse_env_file(&dockerfile_env)?);
                    return Ok(env_vars);
                }
            }
        }
    }
    
    // Fall back to root .env file
    let root_env = app_path.join(".env");
    
    if root_env.exists() {
        env_vars.extend(parse_env_file(&root_env)?);
    }
    
    Ok(env_vars)
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
        
        let env_vars = load_env_vars_for_runtime(temp_dir.path(), None).unwrap();
        assert_eq!(env_vars.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(env_vars.get("KEY2"), Some(&"value2".to_string()));
    }
}

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
fn parse_env_file(env_path: &Path) -> Result<HashMap<String, String>> {
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
            
            env_vars.insert(key, value);
        }
    }
    
    Ok(env_vars)
}

/// Load environment variables with priority:
/// 1. .env file in the same folder as Dockerfile
/// 2. .env file in the root app folder
fn load_env_vars(app_path: &Path, dockerfile_path: &Path) -> Result<HashMap<String, String>> {
    let mut env_vars = HashMap::new();
    
    // First, load root .env (lower priority)
    let root_env = app_path.join(".env");
    if root_env.exists() {
        env_vars.extend(parse_env_file(&root_env)?);
    }
    
    // Then, load .env in Dockerfile directory (higher priority, overwrites root)
    if let Some(dockerfile_dir) = dockerfile_path.parent() {
        let dockerfile_env = dockerfile_dir.join(".env");
        if dockerfile_env.exists() {
            env_vars.extend(parse_env_file(&dockerfile_env)?);
        }
    }
    
    Ok(env_vars)
}

/// Convert environment variables to docker command line flags
/// Returns a string like: "-e KEY1=VALUE1 -e KEY2=VALUE2"
fn env_vars_to_flags(env_vars: &HashMap<String, String>) -> String {
    if env_vars.is_empty() {
        return String::new();
    }
    
    env_vars
        .iter()
        .map(|(key, value)| format!("-e {}={}", key, value))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Phase 2c: Detect OrbStack commands if Dockerfile exists
/// Similar to Docker detection but uses OrbStack context
/// Parses .env files and passes environment variables as command-line flags
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
    
    // Load environment variables from .env files
    let env_vars = load_env_vars(path, dockerfile_path)?;
    let env_flags = env_vars_to_flags(&env_vars);
    
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
            if stage_name.to_lowercase().contains("test") {
                let run_cmd = if env_flags.is_empty() {
                    format!("docker --context orbstack run --rm {}:{}", app_name, stage_name)
                } else {
                    format!("docker --context orbstack run --rm {} {}:{}", env_flags, app_name, stage_name)
                };
                commands.insert(format!("{}-run", stage_name), run_cmd);
            }
        }
    }
    
    // Always add a general build command (builds the final stage)
    commands.insert("build".to_string(), format!("docker --context orbstack build -t {} .", app_name));
    
    // Add run command with appropriate port mappings based on app type
    let base_run_cmd = match app_type {
        "nodejs" | "nx" => {
            // Node.js apps typically run on port 3000
            format!("docker --context orbstack run --name {} --rm -p 3000:3000", app_name)
        }
        "python" => {
            // Python web apps typically run on port 8000
            format!("docker --context orbstack run --name {} --rm -p 8000:8000", app_name)
        }
        "redis" => {
            // Redis default port is 6379
            format!("docker --context orbstack run --name {} --rm -p 6379:6379", app_name)
        }
        "traefik" => {
            // Traefik uses ports 80 (HTTP) and 443 (HTTPS)
            format!("docker --context orbstack run --name {} --rm -p 80:80 -p 443:443", app_name)
        }
        _ => {
            // Unknown app type - no port mapping
            format!("docker --context orbstack run --name {} --rm", app_name)
        }
    };
    
    // Add environment variables and image name to run command
    let run_cmd = if env_flags.is_empty() {
        format!("{} {}", base_run_cmd, app_name)
    } else {
        format!("{} {} {}", base_run_cmd, env_flags, app_name)
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
        writeln!(file, "").unwrap();
        writeln!(file, "KEY4=value4").unwrap();
        file.flush().unwrap();
        
        let env_vars = parse_env_file(file.path()).unwrap();
        assert_eq!(env_vars.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(env_vars.get("KEY2"), Some(&"value2".to_string()));
        assert_eq!(env_vars.get("KEY3"), Some(&"value3".to_string()));
        assert_eq!(env_vars.get("KEY4"), Some(&"value4".to_string()));
    }
    
    #[test]
    fn test_env_vars_to_flags() {
        let mut env_vars = HashMap::new();
        env_vars.insert("KEY1".to_string(), "value1".to_string());
        env_vars.insert("KEY2".to_string(), "value2".to_string());
        
        let flags = env_vars_to_flags(&env_vars);
        assert!(flags.contains("-e KEY1=value1"));
        assert!(flags.contains("-e KEY2=value2"));
    }
    
    #[test]
    fn test_env_vars_to_flags_empty() {
        let env_vars = HashMap::new();
        let flags = env_vars_to_flags(&env_vars);
        assert_eq!(flags, "");
    }
}

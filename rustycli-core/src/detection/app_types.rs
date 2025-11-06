// App type detection module
// Identifies what type of app we're dealing with based on marker files
// Supports: nx, nodejs, python, redis, traefik

use crate::Result;
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Phase 1: Detect what type of app this is
/// Checks for marker files that identify different app types
/// Priority: nx > nodejs > redis > traefik > python
/// 
/// # Arguments
/// * `path` - Directory to check for app type markers
/// 
/// # Returns
/// App type as string (nx, nodejs, python, redis, traefik)
pub fn detect_app_type(path: &Path) -> Result<String> {
    // Check for Nx monorepo (highest priority)
    // nx.json is the marker file for Nx workspaces
    if path.join("nx.json").exists() {
        return Ok("nx".to_string());
    }
    
    // Check for Node.js app (package.json)
    if path.join("package.json").exists() {
        return Ok("nodejs".to_string());
    }
    
    // Check for Redis configurations (multiple patterns)
    if path.join("redis.conf").exists() {
        return Ok("redis".to_string());
    }
    
    // Check for Traefik configurations (multiple patterns)
    if path.join("traefik.yml").exists() 
        || path.join("traefik.yaml").exists() 
        || path.join("traefik.toml").exists() {
        return Ok("traefik".to_string());
    }
    
    // Check all files in directory for additional patterns
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            
            // Additional Traefik patterns
            if file_name_str.starts_with("traefik") && 
               (file_name_str.ends_with(".yml") || 
                file_name_str.ends_with(".yaml") || 
                file_name_str.ends_with(".toml")) {
                return Ok("traefik".to_string());
            }
            
            // Additional Redis patterns
            if file_name_str.starts_with("redis") && 
               (file_name_str.ends_with(".conf") || 
                file_name_str.ends_with(".config")) {
                return Ok("redis".to_string());
            }
            
            // Check docker-compose for redis or traefik services
            if file_name_str == "docker-compose.yml" || file_name_str == "docker-compose.yaml" {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    let content_lower = content.to_lowercase();
                    if content_lower.contains("redis:") || content_lower.contains("image: redis") {
                        return Ok("redis".to_string());
                    }
                    if content_lower.contains("traefik") || content_lower.contains("image: traefik") {
                        return Ok("traefik".to_string());
                    }
                }
            }
        }
    }
    
    // Check subdirectories for Redis and Traefik (common pattern: redis/ folder with configs)
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let dir_name = entry.file_name();
                let dir_name_str = dir_name.to_string_lossy();
                
                // Check for redis/ directory with configs
                if dir_name_str == "redis" {
                    let redis_dir = entry.path();
                    if redis_dir.join("redis.conf").exists() ||
                       redis_dir.join("redis.config").exists() {
                        return Ok("redis".to_string());
                    }
                }
                
                // Check for traefik/ directory with configs
                if dir_name_str == "traefik" {
                    let traefik_dir = entry.path();
                    if traefik_dir.join("traefik.yml").exists() ||
                       traefik_dir.join("traefik.yaml").exists() ||
                       traefik_dir.join("traefik.toml").exists() {
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

/// Extract the app name from configuration files or directory name
/// 
/// # Arguments
/// * `path` - Directory path
/// * `app_type` - The detected app type
/// 
/// # Returns
/// App name as string
pub fn extract_app_name(path: &Path, app_type: &str) -> Result<String> {
    // For Node.js and Nx apps, check package.json for the "name" field
    if app_type == "nodejs" || app_type == "nx" {
        if let Ok(content) = fs::read_to_string(path.join("package.json")) {
            if let Ok(json) = serde_json::from_str::<Value>(&content) {
                if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                    // Handle scoped packages like "@scope/package-name"
                    // We only want the "package-name" part
                    if name.contains('/') {
                        if let Some(short_name) = name.split('/').next_back() {
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

#[cfg(test)]
#[path = "app_types_tests.rs"]
mod app_types_tests;
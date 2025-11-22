// Utility functions for file detection and path operations
// Shared helper functions used across detection modules

use crate::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Search for Dockerfile up to 2 levels deep in the directory tree
/// 
/// # Arguments
/// * `path` - Root directory to search
/// 
/// # Returns
/// Path to Dockerfile if found, None otherwise
pub fn find_dockerfile(path: &Path) -> Result<Option<PathBuf>> {
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

/// Search for Kubernetes manifest files up to 2 levels deep
/// Looks for:
///   - k8s/*.yaml files
///   - *.k8s.yaml files in current directory
///   - *.k8s.yaml files in subdirectories
/// 
/// # Arguments
/// * `path` - Root directory to search
/// 
/// # Returns
/// List of relative paths to k8s manifest files
pub fn find_k8s_files(path: &Path) -> Result<Vec<String>> {
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

#[cfg(test)]
#[path = "utils_tests.rs"]
mod utils_tests;
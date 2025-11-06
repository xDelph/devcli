// Kubernetes environment detection
// Detects Kubernetes commands if manifests exist

use crate::detection::utils::find_k8s_files;
use crate::Result;
use std::collections::HashMap;
use std::path::Path;

/// Phase 2c: Detect Kubernetes commands if manifests exist
/// Searches for k8s/*.yaml files or *.k8s.yaml files up to 2 levels deep
/// 
/// # Arguments
/// * `path` - App directory
/// 
/// # Returns
/// HashMap of kubectl commands, or None if no k8s files found
pub fn detect_k8s_commands(path: &Path) -> Result<Option<HashMap<String, String>>> {
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
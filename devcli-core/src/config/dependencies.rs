// Dependency resolution and checking
// Ensures apps start in the correct order and dependencies are running

use super::models::Config;
use super::resolver::{get_app_by_project, ResolvedApp};
use crate::process::ProcessTracker;
use crate::Result;
use std::collections::{HashSet, VecDeque};

// Resolve the full dependency chain for an app
// Returns all dependencies in the correct order (dependencies-first)
//
// Example: If Worker depends on API, and API depends on Redis
// Returns: [Redis, API]  (Worker itself is NOT included)
//
// Also detects circular dependencies:
// - If A depends on B, and B depends on A → Error
// - If A→B→C→A → Error
//
// Algorithm: Breadth-First Search (BFS) with cycle detection
pub fn resolve_dependency_chain(
    config: &Config,
    resolved_app: &ResolvedApp,
) -> Result<Vec<ResolvedApp>> {
    // Vec to store the final dependency chain
    let mut chain = Vec::new();
    
    // HashSet to track which apps we've already processed
    // Prevents processing the same dependency multiple times
    let mut visited = HashSet::new();
    
    // HashSet to track apps currently being processed
    // Used for detecting circular dependencies
    let mut in_progress = HashSet::new();
    
    // Queue for breadth-first search
    // VecDeque allows efficient push/pop from both ends
    // Stores (key, resolved_app) pairs where key = "project/app"
    let mut queue = VecDeque::new();
    
    // Start with the initial app
    let initial_key = format!("{}/{}", resolved_app.project, resolved_app.app_name);
    queue.push_back((initial_key.clone(), resolved_app.clone()));
    
    // Process the queue until empty
    while let Some((key, current_app)) = queue.pop_front() {
        // If we've already fully processed this app, skip it
        if visited.contains(&key) {
            continue;
        }
        
        // Check for circular dependency
        // If this app is in_progress, we've encountered it again before finishing
        // This means there's a cycle: A→B→...→A
        if in_progress.contains(&key) {
            anyhow::bail!(
                "Circular dependency detected involving '{}/{}'. Dependencies: {:?}",
                current_app.project,
                current_app.app_name,
                current_app.app.dependencies
            );
        }
        
        // Mark this app as currently being processed
        in_progress.insert(key.clone());
        
        // Process each dependency of the current app
        for dep in &current_app.app.dependencies {
            // Build a unique key for this dependency
            let dep_key = format!("{}/{}", dep.project, dep.app);
            
            // Only process if not already visited
            if !visited.contains(&dep_key) {
                // Resolve the dependency app from the config
                // This can fail if the dependency doesn't exist
                let dep_resolved = get_app_by_project(config, &dep.project, &dep.app)?;
                
                // Add to queue for processing
                queue.push_back((dep_key, dep_resolved.clone()));
                
                // Add to dependency chain if not already there
                // .iter().any(...) checks if any item matches the condition
                if !chain.iter().any(|a: &ResolvedApp| {
                    a.project == dep_resolved.project && a.app_name == dep_resolved.app_name
                }) {
                    chain.push(dep_resolved);
                }
            }
        }
        
        // Done processing this app
        // Remove from in_progress and add to visited
        in_progress.remove(&key);
        visited.insert(key);
    }
    
    Ok(chain)
}

// Check which dependencies are NOT currently running
// Returns a list of missing dependencies in "project/app" format
//
// Example: If Redis should be running but isn't, returns ["infrastructure/redis"]
pub fn check_dependencies_running(
    tracker: &ProcessTracker,
    dependencies: &[ResolvedApp],
) -> Result<Vec<String>> {
    // Vec to collect missing dependencies
    let mut missing = Vec::new();
    
    // Check each dependency
    for dep in dependencies {
        // Build the key "project/app" for display
        let app_key = format!("{}/{}", dep.project, dep.app_name);
        
        // Try to get the process info for this dependency
        // .get_process() returns Option<ProcessInfo>
        if let Some(process) = tracker.get_process(&dep.app_name)? {
            // Process info exists, but is it still running?
            // Check if the PID is still active
            if !tracker.is_running(process.pid) {
                // Process died - treat as missing
                missing.push(app_key);
            }
            // else: process is running, all good!
        } else {
            // No process info found - never started
            missing.push(app_key);
        }
    }
    
    Ok(missing)
}

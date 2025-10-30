// App resolution - finding apps in the config and handling ambiguities
// This module helps find apps by name and provides helpful error messages

use super::models::{App, Config};
use crate::Result;

// Container for a resolved app with full context
// When we find an app, we need to know both which project it's in and its data
#[derive(Debug, Clone)]
pub struct ResolvedApp {
    pub project: String,      // Which project contains this app
    pub app_name: String,      // The app's name
    pub app: App,              // The actual app configuration
}

// Find an app by name across all projects
// Handles three cases:
// 1. App found in exactly one project → Success
// 2. App found in multiple projects → Error with list of projects (ambiguous)
// 3. App not found → Error with typo suggestion if possible
//
// project_filter: If Some("project-name"), only search that project
pub fn resolve_app(config: &Config, app_name: &str, project_filter: Option<&str>) -> Result<ResolvedApp> {
    // Vec to collect all matches (project_name, app_config)
    let mut matches = Vec::new();
    
    // Search through all projects
    // .iter() creates an iterator over (key, value) pairs
    for (project_name, project) in &config.projects {
        // If user specified a project, skip others
        if let Some(filter) = project_filter {
            if project_name != filter {
                continue; // Skip this project
            }
        }
        
        // Try to get the app from this project's apps HashMap
        // .get(app_name) returns Option<&App>
        // - Some(&app) if the app exists
        // - None if it doesn't exist
        if let Some(app) = project.apps.get(app_name) {
            // Found it! Add to matches
            // .clone() creates a copy of the app
            matches.push((project_name.clone(), app.clone()));
        }
    }
    
    // Now analyze the matches to decide what to return
    match matches.len() {
        // Case 1: App not found anywhere
        0 => {
            // Try to find a similar app name (typo detection)
            let suggestion = find_similar_app_name(config, app_name);
            
            // If we found a similar name, suggest it
            if let Some(similar) = suggestion {
                anyhow::bail!(
                    "App '{}' not found in config. Did you mean '{}'?",
                    app_name,
                    similar
                );
            } else {
                // No similar names, just say it wasn't found
                anyhow::bail!("App '{}' not found in config.", app_name);
            }
        }
        
        // Case 2: Found exactly one match - this is what we want!
        1 => {
            // Extract the single match from the vector
            // .into_iter() converts Vec into an iterator that takes ownership
            // .next() gets the first item
            // .unwrap() is safe because we know len==1
            let (project, app) = matches.into_iter().next().unwrap();
            
            // Return the resolved app with full context
            Ok(ResolvedApp {
                project,
                app_name: app_name.to_string(),
                app,
            })
        }
        
        // Case 3: Found multiple matches - ambiguous!
        _ => {
            // Build a list of project names where we found the app
            // .iter() = iterate over matches
            // .map(|(p, _)| p.clone()) = extract just the project name
            // .collect() = gather into a Vec<String>
            let project_list: Vec<String> = matches.iter().map(|(p, _)| p.clone()).collect();
            
            // Tell the user they need to specify which project
            anyhow::bail!(
                "App name '{}' is ambiguous. Found in projects: {}. Use --project to specify.",
                app_name,
                project_list.join(", ") // Join with commas: "proj1, proj2, proj3"
            );
        }
    }
}

// Get an app from a specific project
// Similar to resolve_app but requires both project and app name
// No ambiguity possible since we know exactly which one to get
pub fn get_app_by_project(config: &Config, project: &str, app_name: &str) -> Result<ResolvedApp> {
    // Try to get the project
    // .get(project) returns Option<&Project>
    // .ok_or_else() converts None to an error
    let proj = config
        .projects
        .get(project)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found in config.", project))?;
    
    // Try to get the app from that project
    let app = proj
        .apps
        .get(app_name)
        .ok_or_else(|| anyhow::anyhow!("App '{}' not found in project '{}'.", app_name, project))?;
    
    // Success! Return the resolved app
    Ok(ResolvedApp {
        project: project.to_string(),
        app_name: app_name.to_string(),
        app: app.clone(),
    })
}

// Get a flat list of all apps across all projects
// Returns: Vec of (project_name, app_name, app_config) tuples
// Useful for listing all available apps
pub fn list_all_apps(config: &Config) -> Vec<(String, String, App)> {
    let mut apps = Vec::new();
    
    // Nested loops to iterate through all projects and all apps
    for (project_name, project) in &config.projects {
        for (app_name, app) in &project.apps {
            // Add this app to the list with full context
            apps.push((project_name.clone(), app_name.clone(), app.clone()));
        }
    }
    
    apps
}

// Find an app name similar to the target (for typo suggestions)
// Uses Levenshtein distance to measure similarity
// Returns the closest match if distance <= 2
//
// Examples of distance 2 or less:
// - "api-privat" vs "api-private" (distance = 1, missing 'e')
// - "luve-api" vs "luce-api" (distance = 1, v→c)
fn find_similar_app_name(config: &Config, target: &str) -> Option<String> {
    // Get all app names
    let all_apps = list_all_apps(config);
    
    // Find the first app name with distance <= 2
    // .iter() = iterate over the apps
    // .map(|(_, app_name, _)| app_name) = extract just the app name
    // .find(|&app_name| ...) = find the first one matching the condition
    // .cloned() = convert &String to String
    all_apps
        .iter()
        .map(|(_, app_name, _)| app_name)
        .find(|&app_name| levenshtein_distance(app_name, target) <= 2)
        .cloned()
}

// Calculate Levenshtein distance between two strings
// This measures how many single-character edits (insert, delete, replace)
// are needed to transform s1 into s2
//
// Examples:
// - levenshtein("cat", "hat") = 1 (replace c with h)
// - levenshtein("saturday", "sunday") = 3
// - levenshtein("api", "api") = 0 (identical)
//
// Algorithm: Dynamic programming (builds a 2D table)
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();
    
    // Create a 2D matrix to store distances
    // matrix[i][j] = distance between first i chars of s1 and first j chars of s2
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];
    
    // Initialize first column: distance from empty string to prefixes of s1
    // matrix[i][0] = i (need i deletions)
    #[allow(clippy::needless_range_loop)]
    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    
    // Initialize first row: distance from empty string to prefixes of s2
    // matrix[0][j] = j (need j insertions)
    for j in 0..=len2 {
        matrix[0][j] = j;
    }
    
    // Fill in the rest of the matrix
    // .enumerate() gives us (index, character) pairs
    for (i, c1) in s1.chars().enumerate() {
        for (j, c2) in s2.chars().enumerate() {
            // If characters match, no cost. Otherwise, cost of 1 (replacement)
            let cost = if c1 == c2 { 0 } else { 1 };
            
            // Matrix is 1-indexed (we have the extra row/col for empty string)
            // Three options:
            // 1. Delete from s1: matrix[i][j+1] + 1
            // 2. Insert into s1: matrix[i+1][j] + 1
            // 3. Replace/match: matrix[i][j] + cost
            // Take the minimum of these three
            matrix[i + 1][j + 1] = std::cmp::min(
                std::cmp::min(
                    matrix[i][j + 1] + 1,      // deletion
                    matrix[i + 1][j] + 1        // insertion
                ),
                matrix[i][j] + cost,             // replacement/match
            );
        }
    }
    
    // The answer is in the bottom-right corner
    // This represents the distance between the full strings
    matrix[len1][len2]
}

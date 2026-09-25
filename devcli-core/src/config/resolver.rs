// App resolution - finding apps in the config and handling ambiguities
// Core matching/fuzzy logic lives in config_manager_support::ProjectAppResolver.
// This module keeps the public API and interactive ambiguity prompt.

use super::models::{App, Config};
use crate::config_manager_support::{map_resolve_error, ProjectAppResolver};
use crate::Result;
use config_manager::resolver::Resolver;
use inquire::Select;

// Container for a resolved app with full context
// When we find an app, we need to know both which project it's in and its data
#[derive(Debug, Clone)]
pub struct ResolvedApp {
    pub project: String,  // Which project contains this app
    pub app_name: String, // The app's name
    pub app: App,         // The actual app configuration
}

// Find an app by name across all projects
// Handles three cases:
// 1. App found in exactly one project → Success
// 2. App found in multiple projects → interactive Select (or error if cancelled)
// 3. App not found → Error with typo suggestion if possible
//
// project_filter: If Some("project-name"), only search that project
pub fn resolve_app(
    config: &Config,
    app_name: &str,
    project_filter: Option<&str>,
) -> Result<ResolvedApp> {
    let resolver = ProjectAppResolver::new();

    match resolver.resolve_with_filter(config, app_name, project_filter) {
        Ok(resolved) => Ok(resolved),
        Err(config_manager::Error::Ambiguous { candidates, .. }) => {
            let prompt_message =
                format!("App '{app_name}' found in multiple projects. Please select one:");

            let selection = Select::new(&prompt_message, candidates.clone()).prompt();

            match selection {
                Ok(selected_project) => resolver
                    .resolve_with_filter(config, app_name, Some(&selected_project))
                    .map_err(|err| map_resolve_error(err, app_name)),
                Err(_) => Err(anyhow::anyhow!(
                    "App name '{}' is ambiguous. Found in projects: {}. Use --project to specify.",
                    app_name,
                    candidates.join(", ")
                )),
            }
        }
        Err(err) => Err(map_resolve_error(err, app_name)),
    }
}

// Get an app from a specific project
// Similar to resolve_app but requires both project and app name
// No ambiguity possible since we know exactly which one to get
pub fn get_app_by_project(config: &Config, project: &str, app_name: &str) -> Result<ResolvedApp> {
    let resolver = ProjectAppResolver::new();
    match resolver.resolve(config, &format!("{project}/{app_name}")) {
        Ok(resolved) => Ok(resolved),
        Err(config_manager::Error::NotFound { .. }) => {
            // Preserve the more specific project/app error messages
            if !config.projects.contains_key(project) {
                anyhow::bail!("Project '{}' not found in config.", project);
            }
            anyhow::bail!("App '{}' not found in project '{}'.", app_name, project);
        }
        Err(err) => Err(map_resolve_error(err, app_name)),
    }
}

// Get a flat list of all apps across all projects
// Returns: Vec of (project_name, app_name, app_config) tuples
// Useful for listing all available apps
pub fn list_all_apps(config: &Config) -> Vec<(String, String, App)> {
    let mut apps = Vec::new();

    for (project_name, project) in &config.projects {
        for (app_name, app) in &project.apps {
            apps.push((project_name.clone(), app_name.clone(), app.clone()));
        }
    }

    apps
}

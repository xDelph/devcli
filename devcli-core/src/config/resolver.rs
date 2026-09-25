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

            // The user has already told us which project owns the app — pick it
            // directly from the config instead of re-running a second resolution
            // pass (the old code called `resolve_with_filter` again).
            match selection {
                Ok(selected_project) => pick_app_in_project(config, &selected_project, app_name),
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
/// Pick an app that is already known to live in `project` after an ambiguity
/// prompt — no second search pass.
///
/// Mirrors `ProjectAppResolver::resolve_in_project`: exact key first, then
/// `alternative_name`. The caller knows the app exists here (it came from the
/// initial resolution's candidate list), so failures are defensive only.
pub fn pick_app_in_project(config: &Config, project: &str, app_name: &str) -> Result<ResolvedApp> {
    let proj = config
        .projects
        .get(project)
        .ok_or_else(|| anyhow::anyhow!("Project '{project}' not found in config."))?;

    if let Some(app) = proj.apps.get(app_name) {
        return Ok(ResolvedApp {
            project: project.to_string(),
            app_name: app_name.to_string(),
            app: app.clone(),
        });
    }

    if let Some((actual_name, app)) = proj
        .apps
        .iter()
        .find(|(_, a)| a.alternative_name.as_deref() == Some(app_name))
    {
        return Ok(ResolvedApp {
            project: project.to_string(),
            app_name: actual_name.clone(),
            app: app.clone(),
        });
    }

    anyhow::bail!("App '{app_name}' not found in project '{project}'.")
}

pub fn list_all_apps(config: &Config) -> Vec<(String, String, App)> {
    let mut apps = Vec::new();

    for (project_name, project) in &config.projects {
        for (app_name, app) in &project.apps {
            apps.push((project_name.clone(), app_name.clone(), app.clone()));
        }
    }

    apps
}

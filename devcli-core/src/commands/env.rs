// Environment files management commands
// Allows users to add, remove, and configure env files for apps

use crate::config::models::{Environment, Stage};
use crate::config::{load_config, resolve_app, save_config, App, Config};
use crate::output;
use crate::Result;
use anyhow::anyhow;
use std::collections::{BTreeMap, HashMap};

fn validate_stage(stage: &str) -> Result<()> {
    if Stage::from_string(stage).is_none() {
        anyhow::bail!(
            "Invalid stage '{stage}'. Must be one of: {} (or `base` for env files of every stage)",
            Stage::all_names()
        );
    }
    Ok(())
}

/// The canonical name of a `--context` (an environment; aliases accepted).
fn parse_context(context: &str) -> Result<&'static str> {
    Environment::parse(context)
        .map(|env| env.as_str())
        .map_err(|e| anyhow!("Invalid context: {e}"))
}

/// The app `app_name` designates, resolved like every other command does it:
/// `alternative_name`, `project/app` and `--project` are understood, and a
/// name several projects use is an error (or a prompt, on a terminal) instead
/// of the first app that happened to have that key.
///
/// Returns its project and real app key.
fn locate(config: &Config, app_name: &str, project: Option<&str>) -> Result<(String, String)> {
    let resolved = resolve_app(config, app_name, project)?;
    Ok((resolved.project, resolved.app_name))
}

fn app_mut<'a>(config: &'a mut Config, project: &str, app_name: &str) -> Result<&'a mut App> {
    config
        .projects
        .get_mut(project)
        .and_then(|p| p.apps.get_mut(app_name))
        .ok_or_else(|| anyhow!("App '{app_name}' not found in project '{project}'"))
}

/// Add an environment file to an app's configuration
///
/// # Arguments
/// * `app_name` - Name of the app to configure (or `project/app`, or its alternative name)
/// * `project` - Project of the app, when the name alone is ambiguous
/// * `stage` - Stage name (`dev`, `qa`, `preprod`, `prod`, or `base`)
/// * `context` - Environment the file is for (`local`, `docker`, ...; aliases accepted)
/// * `file_path` - Path to the env file relative to app root
///
/// Only the given context is written. It used to also overwrite the docker,
/// orbstack and docker-compose entries of the stage when given one of them,
/// while `env remove` removed only the one named, so the two did not undo
/// each other.
pub fn add_env_file(
    app_name: &str,
    project: Option<&str>,
    stage: &str,
    context: &str,
    file_path: &str,
) -> Result<()> {
    // Any other stage was saved, and could then never be used: `start
    // --stage` only knows these. `base` applies to every stage.
    if stage != "base" {
        validate_stage(stage)?;
    }
    let context = parse_context(context)?;
    if file_path.trim().is_empty() {
        anyhow::bail!("The env file path must not be empty");
    }

    let mut config = load_config()?;
    let (project, app_name) = locate(&config, app_name, project)?;
    let app = app_mut(&mut config, &project, &app_name)?;

    app.env_files
        .get_or_insert_with(HashMap::new)
        .entry(stage.to_string())
        .or_default()
        .insert(context.to_string(), file_path.to_string());

    save_config(&config)?;

    if output::json_enabled() {
        return output::print_json(&serde_json::json!({
            "project": project,
            "app": app_name,
            "stage": stage,
            "context": context,
            "file": file_path,
        }));
    }
    println!("✓ Added env file for {project}/{app_name}");
    println!("  Stage: {stage}");
    println!("  Context: {context}");
    println!("  File: {file_path}");
    Ok(())
}

/// Remove an environment file or entire stage from an app's configuration
///
/// # Arguments
/// * `app_name` - Name of the app to configure
/// * `project` - Project of the app, when the name alone is ambiguous
/// * `stage` - Stage name to remove
/// * `context` - Optional context name. If None, removes entire stage
pub fn remove_env_file(
    app_name: &str,
    project: Option<&str>,
    stage: &str,
    context: Option<&str>,
) -> Result<()> {
    let mut config = load_config()?;
    let (project, app_name) = locate(&config, app_name, project)?;
    let app = app_mut(&mut config, &project, &app_name)?;

    let env_files = app
        .env_files
        .as_mut()
        .ok_or_else(|| anyhow!("No env files configured for app '{project}/{app_name}'"))?;
    let stage_map = env_files
        .get_mut(stage)
        .ok_or_else(|| anyhow!("Stage '{stage}' not found for app '{project}/{app_name}'"))?;

    // The canonical name when it is one (`compose` removes `docker-compose`);
    // any other key is taken as written, so an entry that an older devcli
    // saved under a bogus name can still be removed.
    let context = context.map(Environment::canonical_name);
    let removed: BTreeMap<String, String> = match &context {
        Some(ctx) => {
            let file = stage_map.remove(ctx).ok_or_else(|| {
                anyhow!(
                    "No env file for context '{ctx}' in stage '{stage}' of '{project}/{app_name}'"
                )
            })?;
            BTreeMap::from([(ctx.clone(), file)])
        }
        None => std::mem::take(stage_map).into_iter().collect(),
    };

    if stage_map.is_empty() {
        env_files.remove(stage);
    }
    if env_files.is_empty() {
        app.env_files = None;
    }

    save_config(&config)?;

    if output::json_enabled() {
        return output::print_json(&serde_json::json!({
            "project": project,
            "app": app_name,
            "stage": stage,
            "context": context,
            "removed": removed,
        }));
    }
    match &context {
        Some(ctx) => {
            println!("✓ Removed the {ctx} env file of stage {stage} from {project}/{app_name}")
        }
        None => println!("✓ Removed stage {stage} from {project}/{app_name}"),
    }
    Ok(())
}

/// List all environment files configured for an app
///
/// # Arguments
/// * `app_name` - Name of the app to list env files for
/// * `project` - Project of the app, when the name alone is ambiguous
pub fn list_env_files(app_name: &str, project: Option<&str>) -> Result<()> {
    let config = load_config()?;
    let resolved = resolve_app(&config, app_name, project)?;
    let (project_name, app_name, app) = (resolved.project, resolved.app_name, resolved.app);

    // Sorted maps, for a stable display and JSON answer.
    let env_files: BTreeMap<&String, BTreeMap<&String, &String>> = app
        .env_files
        .iter()
        .flatten()
        .map(|(stage, contexts)| (stage, contexts.iter().collect()))
        .collect();
    let default_stages: BTreeMap<&String, &String> = app.default_stages.iter().flatten().collect();

    if output::json_enabled() {
        return output::print_json(&serde_json::json!({
            "project": project_name,
            "app": app_name,
            "env_files": env_files,
            "default_stages": default_stages,
        }));
    }

    println!("Environment files for {}/{}:", project_name, app_name);
    println!();

    if env_files.is_empty() {
        println!("  No env files configured");
    }
    for (stage, contexts) in &env_files {
        println!("  {}:", stage);
        for (context, file_path) in contexts {
            println!("    • {}: {}", context, file_path);
        }
        println!();
    }

    if !default_stages.is_empty() {
        println!("Default stages:");
        for (context, stage) in &default_stages {
            println!("  • {}: {}", context, stage);
        }
    }

    Ok(())
}

/// Set default stage for an app's context
///
/// # Arguments
/// * `app_name` - Name of the app to configure
/// * `project` - Project of the app, when the name alone is ambiguous
/// * `context` - Context name (e.g., "local", "docker", "orbstack", "k8s")
/// * `stage` - Stage name to set as default
pub fn set_default_stage(
    app_name: &str,
    project: Option<&str>,
    context: &str,
    stage: &str,
) -> Result<()> {
    validate_stage(stage)?;
    let context = parse_context(context)?;
    let mut config = load_config()?;
    let (project, app_name) = locate(&config, app_name, project)?;
    let app = app_mut(&mut config, &project, &app_name)?;

    app.default_stages
        .get_or_insert_with(HashMap::new)
        .insert(context.to_string(), stage.to_string());

    save_config(&config)?;

    if output::json_enabled() {
        return output::print_json(&serde_json::json!({
            "project": project,
            "app": app_name,
            "context": context,
            "stage": stage,
        }));
    }
    println!("✓ Set default stage for {}/{}", project, app_name);
    println!("  Context: {}", context);
    println!("  Default stage: {}", stage);

    Ok(())
}

#[cfg(test)]
#[path = "env_test.rs"]
mod tests;

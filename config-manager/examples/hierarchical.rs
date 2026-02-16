//! Hierarchical configuration example.
//!
//! Demonstrates a devcli-like hierarchical structure with projects and apps.

use config_manager::prelude::*;
use config_manager::resolver::HashMapResolver;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    projects: HashMap<String, Project>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Project {
    apps: HashMap<String, App>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct App {
    name: String,
    path: String,
    port: u16,
}

fn main() -> Result<()> {
    // Create a configuration manager
    let manager = ConfigManager::<Config>::builder()
        .loader(JsonLoader::new("examples/hierarchical_config.json"))
        .build()?;

    // Create a sample hierarchical config
    let mut config = Config {
        projects: HashMap::new(),
    };

    let mut web_apps = HashMap::new();
    web_apps.insert(
        "frontend".to_string(),
        App {
            name: "Frontend".to_string(),
            path: "~/projects/frontend".to_string(),
            port: 3000,
        },
    );
    web_apps.insert(
        "backend".to_string(),
        App {
            name: "Backend API".to_string(),
            path: "~/projects/backend".to_string(),
            port: 8080,
        },
    );

    config
        .projects
        .insert("web".to_string(), Project { apps: web_apps });

    // Save the config
    manager.save(&config)?;
    println!("✓ Saved hierarchical configuration");

    // Load it back
    let loaded = manager.load()?;
    println!("✓ Loaded configuration:");

    for (project_name, project) in &loaded.projects {
        println!("\n  Project: {}", project_name);
        for (app_name, app) in &project.apps {
            println!("    - {} ({}:{})", app.name, app_name, app.port);
        }
    }

    // Demonstrate resolving an app
    if let Some(project) = loaded.projects.get("web") {
        let resolver = HashMapResolver::new(true);
        match resolver.resolve(&project.apps, "frontend") {
            Ok(app) => println!(
                "\n✓ Resolved 'frontend' app: {} on port {}",
                app.name, app.port
            ),
            Err(e) => println!("\n✗ Failed to resolve: {}", e),
        }
    }

    Ok(())
}

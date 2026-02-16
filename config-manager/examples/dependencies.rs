//! Dependency resolution example.
//!
//! Demonstrates dependency chain resolution and cycle detection.

use config_manager::dependencies::{DependencyGraph, DependencyProvider};
use config_manager::prelude::*;
use config_manager::resolver::Resolver;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    apps: HashMap<String, App>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct App {
    name: String,
    depends_on: Vec<String>,
}

struct AppDependencyProvider;

impl DependencyProvider<App> for AppDependencyProvider {
    fn dependencies(&self, entity: &App) -> Vec<String> {
        entity.depends_on.clone()
    }
}

struct AppResolver;

impl Resolver<Config, App> for AppResolver {
    fn resolve(&self, config: &Config, id: &str) -> config_manager::core::Result<App> {
        config
            .apps
            .get(id)
            .cloned()
            .ok_or_else(|| config_manager::core::Error::NotFound {
                id: id.to_string(),
                suggestion: None,
            })
    }

    fn list_ids(&self, config: &Config) -> Vec<String> {
        config.apps.keys().cloned().collect()
    }
}

fn main() -> Result<()> {
    // Create a sample config with dependencies
    let mut config = Config {
        apps: HashMap::new(),
    };

    config.apps.insert(
        "database".to_string(),
        App {
            name: "Database".to_string(),
            depends_on: vec![],
        },
    );

    config.apps.insert(
        "cache".to_string(),
        App {
            name: "Cache".to_string(),
            depends_on: vec![],
        },
    );

    config.apps.insert(
        "api".to_string(),
        App {
            name: "API".to_string(),
            depends_on: vec!["database".to_string(), "cache".to_string()],
        },
    );

    config.apps.insert(
        "worker".to_string(),
        App {
            name: "Worker".to_string(),
            depends_on: vec!["api".to_string()],
        },
    );

    // Create dependency graph
    let graph = DependencyGraph::new(
        Box::new(AppDependencyProvider),
        Box::new(AppResolver),
        Box::new(|app: &App| app.name.clone()),
    );

    // Resolve dependency chain for worker
    let worker = config.apps.get("worker").unwrap();
    match graph.resolve_chain(&config, worker) {
        Ok(chain) => {
            println!("✓ Dependency chain for 'worker':");
            for app in &chain {
                println!("  - {}", app.name);
            }
            println!("  (Total: {} dependencies)", chain.len());
        }
        Err(e) => println!("✗ Error resolving dependencies: {}", e),
    }

    // Check for cycles
    let all_apps: Vec<App> = config.apps.values().cloned().collect();
    match graph.check_cycles(&config, &all_apps) {
        Ok(()) => println!("\n✓ No circular dependencies detected"),
        Err(e) => println!("\n✗ Circular dependency: {}", e),
    }

    Ok(())
}

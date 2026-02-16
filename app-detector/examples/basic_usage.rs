//! Basic usage example for app-detector
//!
//! This example demonstrates:
//! - Creating a detection engine with default strategies
//! - Running detection on a directory
//! - Inspecting the detection report

use app_detector::{engine::DetectionEngine, registry::StrategyRegistry};
use std::env;

fn main() {
    // Get the directory to analyze from command line args or use current directory
    let dir = env::args()
        .nth(1)
        .unwrap_or_else(|| ".".to_string());

    println!("Analyzing directory: {}\n", dir);

    // Create a registry with all built-in detection strategies
    let registry = StrategyRegistry::with_defaults();

    // Create the detection engine
    let engine = DetectionEngine::new(registry);

    // Run detection on the directory
    match engine.detect(&dir) {
        Ok(report) => {
            println!("Detection Report");
            println!("================\n");

            if report.results.is_empty() {
                println!("No project types detected.");
                return;
            }

            // Print summary
            println!("Detected {} project type(s):\n", report.results.len());

            // Print each detection result
            for result in &report.results {
                println!("Strategy: {}", result.strategy_id);
                println!("Category: {:?}", result.category);
                println!("Confidence: {:.0}%", result.confidence * 100.0);

                // Print detailed information based on data type
                use app_detector::types::DetectionData;
                match &result.data {
                    DetectionData::Language(info) => {
                        println!("Language: {}", info.name);
                        if let Some(ref version) = info.version {
                            println!("Version: {}", version);
                        }
                        println!("Files: {} source files", info.primary_files.len());

                        // Print metadata
                        if !info.metadata.is_empty() {
                            println!("Metadata:");
                            for (key, value) in &info.metadata {
                                println!("  {}: {}", key, value);
                            }
                        }
                    }
                    DetectionData::Framework(info) => {
                        println!("Framework: {}", info.name);
                        if let Some(ref version) = info.version {
                            println!("Version: {}", version);
                        }
                    }
                    DetectionData::DockerEnv(info) => {
                        println!("Dockerfiles: {}", info.dockerfiles.len());
                        if !info.base_images.is_empty() {
                            println!("Base images: {}", info.base_images.join(", "));
                        }
                        if !info.exposed_ports.is_empty() {
                            println!(
                                "Exposed ports: {}",
                                info.exposed_ports
                                    .iter()
                                    .map(|p| p.to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                        if !info.commands.is_empty() {
                            println!("Commands: {}", info.commands.keys().cloned().collect::<Vec<_>>().join(", "));
                        }
                    }
                    DetectionData::LocalEnv(info) => {
                        println!("Local commands: {}", info.commands.len());
                        if let Some(ref default) = info.suggested_default {
                            println!("Suggested: {}", default);
                        }
                    }
                    DetectionData::KubernetesEnv(info) => {
                        println!("K8s manifests: {}", info.manifests.len());
                        if !info.helm_charts.is_empty() {
                            println!("Helm charts: {}", info.helm_charts.len());
                        }
                    }
                    DetectionData::OrbStackEnv(info) => {
                        println!("OrbStack commands: {}", info.commands.len());
                    }
                    DetectionData::PackageManager(info) => {
                        println!("Package Manager: {}", info.name);
                        if let Some(ref version) = info.version {
                            println!("Version: {}", version);
                        }
                    }
                    _ => {}
                }

                // Print suggested strategies
                if !result.suggested_strategies.is_empty() {
                    println!(
                        "Suggested strategies: {}",
                        result.suggested_strategies.join(", ")
                    );
                }

                println!();
            }
        }
        Err(e) => {
            eprintln!("Error during detection: {}", e);
            std::process::exit(1);
        }
    }
}

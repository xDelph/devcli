//! Strategy registry for managing detection strategies

use crate::{strategy::DetectionStrategy, types::StrategyCategory};
use std::collections::HashMap;

/// Central registry for all detection strategies
pub struct StrategyRegistry {
    strategies: Vec<Box<dyn DetectionStrategy>>,
    by_id: HashMap<String, usize>,
    by_category: HashMap<StrategyCategory, Vec<usize>>,
}

impl StrategyRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
            by_id: HashMap::new(),
            by_category: HashMap::new(),
        }
    }

    /// Create a registry with all built-in strategies
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();

        // ===== App Type Strategies (Priority 0-300) =====

        // Priority 50: Environment files and monorepo (run first)
        registry.register(Box::new(crate::strategies::EnvFilesStrategy));
        registry.register(Box::new(crate::strategies::NxStrategy));

        // Priority 100: Languages
        registry.register(Box::new(crate::strategies::RustStrategy));
        registry.register(Box::new(crate::strategies::NodeJsStrategy));
        registry.register(Box::new(crate::strategies::PythonStrategy));

        // Priority 150: Services
        registry.register(Box::new(crate::strategies::RedisStrategy));
        registry.register(Box::new(crate::strategies::TraefikStrategy));

        // ===== Environment Capability Strategies (Priority 400-600) =====

        // Priority 400: Docker
        registry.register(Box::new(crate::strategies::DockerStrategy));

        // Priority 410: OrbStack
        registry.register(Box::new(crate::strategies::OrbStackEnvStrategy));

        // Priority 450: Kubernetes
        registry.register(Box::new(crate::strategies::KubernetesEnvStrategy));

        // Priority 500: Local (runs last, depends on app types)
        registry.register(Box::new(crate::strategies::LocalEnvStrategy));

        registry
    }

    /// Register a detection strategy
    pub fn register(&mut self, strategy: Box<dyn DetectionStrategy>) {
        let id = strategy.id().to_string();
        let category = strategy.category();
        let index = self.strategies.len();

        self.by_id.insert(id.clone(), index);
        self.by_category.entry(category).or_default().push(index);
        self.strategies.push(strategy);
    }

    /// Get strategy by ID
    pub fn get(&self, id: &str) -> Option<&dyn DetectionStrategy> {
        self.by_id
            .get(id)
            .and_then(|&idx| self.strategies.get(idx))
            .map(|s| s.as_ref())
    }

    /// Get all strategies in a category
    pub fn by_category(&self, category: &StrategyCategory) -> Vec<&dyn DetectionStrategy> {
        self.by_category
            .get(category)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&idx| self.strategies.get(idx))
                    .map(|s| s.as_ref())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all registered strategies
    pub fn all(&self) -> Vec<&dyn DetectionStrategy> {
        self.strategies.iter().map(|s| s.as_ref()).collect()
    }
}

impl Default for StrategyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

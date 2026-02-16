//! Dependency graph implementation with cycle detection.

use crate::core::{Error, Result};
use crate::dependencies::DependencyProvider;
use crate::resolver::Resolver;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Debug;
use std::hash::Hash;

/// A dependency graph that can resolve dependency chains and detect cycles.
///
/// # Type Parameters
///
/// * `C` - Configuration type
/// * `E` - Entity type
/// * `I` - Identifier type (must be hashable)
///
/// # Examples
///
/// ```rust
/// use config_manager::dependencies::{DependencyGraph, DependencyProvider};
/// use config_manager::resolver::Resolver;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct App {
///     name: String,
///     depends_on: Vec<String>,
/// }
///
/// struct AppProvider;
/// impl DependencyProvider<App> for AppProvider {
///     fn dependencies(&self, entity: &App) -> Vec<String> {
///         entity.depends_on.clone()
///     }
/// }
/// ```
pub struct DependencyGraph<C, E, I = String>
where
    I: Eq + Hash + Clone + Debug,
{
    provider: Box<dyn DependencyProvider<E>>,
    resolver: Box<dyn Resolver<C, E>>,
    id_extractor: Box<dyn Fn(&E) -> I + Send + Sync>,
}

impl<C, E, I> DependencyGraph<C, E, I>
where
    C: Send + Sync,
    E: Clone + Send + Sync,
    I: Eq + Hash + Clone + Debug + ToString,
{
    /// Create a new dependency graph.
    ///
    /// # Arguments
    ///
    /// * `provider` - Extracts dependencies from entities
    /// * `resolver` - Resolves entity identifiers to entities
    /// * `id_extractor` - Extracts identifier from entity
    pub fn new(
        provider: Box<dyn DependencyProvider<E>>,
        resolver: Box<dyn Resolver<C, E>>,
        id_extractor: Box<dyn Fn(&E) -> I + Send + Sync>,
    ) -> Self {
        Self {
            provider,
            resolver,
            id_extractor,
        }
    }

    /// Resolve the full dependency chain for an entity.
    ///
    /// Returns all transitive dependencies in correct dependency order
    /// (dependencies before dependents).
    ///
    /// # Errors
    ///
    /// * `Error::CircularDependency` - Circular dependency detected
    /// * `Error::NotFound` - Dependency not found
    pub fn resolve_chain(&self, config: &C, entity: &E) -> Result<Vec<E>> {
        let mut visited = HashSet::new();
        let mut in_progress = HashSet::new();
        let mut result = Vec::new();
        let mut path = Vec::new();

        self.visit(
            config,
            entity,
            &mut visited,
            &mut in_progress,
            &mut result,
            &mut path,
        )?;

        Ok(result)
    }

    /// Recursive DFS visit for dependency resolution.
    fn visit(
        &self,
        config: &C,
        entity: &E,
        visited: &mut HashSet<I>,
        in_progress: &mut HashSet<I>,
        result: &mut Vec<E>,
        path: &mut Vec<String>,
    ) -> Result<()> {
        let id = (self.id_extractor)(entity);
        let id_str = id.to_string();

        // Check if we've completed this entity
        if visited.contains(&id) {
            return Ok(());
        }

        // Check for circular dependency
        if in_progress.contains(&id) {
            path.push(id_str.clone());
            return Err(Error::CircularDependency { path: path.clone() });
        }

        // Mark as in progress
        in_progress.insert(id.clone());
        path.push(id_str);

        // Visit all dependencies
        for dep_id in self.provider.dependencies(entity) {
            let dep_entity = self.resolver.resolve(config, &dep_id)?;
            self.visit(config, &dep_entity, visited, in_progress, result, path)?;
        }

        // Done with this entity
        in_progress.remove(&id);
        path.pop();
        visited.insert(id);
        result.push(entity.clone());

        Ok(())
    }

    /// Topologically sort a list of entities based on their dependencies.
    ///
    /// Returns entities in an order where all dependencies come before dependents.
    ///
    /// Uses Kahn's algorithm for topological sorting.
    pub fn topological_sort(&self, _config: &C, entities: &[E]) -> Result<Vec<E>> {
        let mut in_degree: HashMap<I, usize> = HashMap::new();
        let mut adj_list: HashMap<I, Vec<E>> = HashMap::new();
        let mut id_to_entity: HashMap<I, E> = HashMap::new();

        // Build graph
        for entity in entities {
            let id = (self.id_extractor)(entity);
            id_to_entity.insert(id.clone(), entity.clone());
            in_degree.entry(id.clone()).or_insert(0);

            for dep_id_str in self.provider.dependencies(entity) {
                // Find the dependency entity
                let dep_entity = entities
                    .iter()
                    .find(|e| (self.id_extractor)(e).to_string() == dep_id_str)
                    .ok_or_else(|| {
                        Error::DependencyError(format!(
                            "Dependency '{}' not found in entity list",
                            dep_id_str
                        ))
                    })?;

                let dep_id = (self.id_extractor)(dep_entity);

                adj_list
                    .entry(dep_id.clone())
                    .or_default()
                    .push(entity.clone());

                *in_degree.entry(id.clone()).or_insert(0) += 1;
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<I> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut result = Vec::new();

        while let Some(id) = queue.pop_front() {
            if let Some(entity) = id_to_entity.get(&id) {
                result.push(entity.clone());

                if let Some(dependents) = adj_list.get(&id) {
                    for dependent in dependents {
                        let dep_id = (self.id_extractor)(dependent);
                        if let Some(degree) = in_degree.get_mut(&dep_id) {
                            *degree -= 1;
                            if *degree == 0 {
                                queue.push_back(dep_id);
                            }
                        }
                    }
                }
            }
        }

        // Check for cycles
        if result.len() != entities.len() {
            return Err(Error::CircularDependency {
                path: vec!["<cycle detected>".to_string()],
            });
        }

        Ok(result)
    }

    /// Check if there are any circular dependencies in a configuration.
    ///
    /// Returns Ok(()) if no cycles, Error::CircularDependency if cycles exist.
    pub fn check_cycles(&self, config: &C, entities: &[E]) -> Result<()> {
        let mut visited = HashSet::new();
        let mut in_progress = HashSet::new();
        let mut path = Vec::new();

        for entity in entities {
            let id = (self.id_extractor)(entity);
            if !visited.contains(&id) {
                let mut result = Vec::new();
                self.visit(
                    config,
                    entity,
                    &mut visited,
                    &mut in_progress,
                    &mut result,
                    &mut path,
                )?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dependencies::DependencyProvider;
    use crate::resolver::Resolver;

    #[derive(Debug, Clone, PartialEq)]
    struct TestEntity {
        id: String,
        deps: Vec<String>,
    }

    struct TestProvider;
    impl DependencyProvider<TestEntity> for TestProvider {
        fn dependencies(&self, entity: &TestEntity) -> Vec<String> {
            entity.deps.clone()
        }
    }

    struct TestResolver;
    impl Resolver<Vec<TestEntity>, TestEntity> for TestResolver {
        fn resolve(&self, config: &Vec<TestEntity>, id: &str) -> Result<TestEntity> {
            config
                .iter()
                .find(|e| e.id == id)
                .cloned()
                .ok_or_else(|| Error::NotFound {
                    id: id.to_string(),
                    suggestion: None,
                })
        }

        fn list_ids(&self, config: &Vec<TestEntity>) -> Vec<String> {
            config.iter().map(|e| e.id.clone()).collect()
        }
    }

    fn create_graph() -> DependencyGraph<Vec<TestEntity>, TestEntity, String> {
        DependencyGraph::new(
            Box::new(TestProvider),
            Box::new(TestResolver),
            Box::new(|e: &TestEntity| e.id.clone()),
        )
    }

    #[test]
    fn test_resolve_chain_no_deps() {
        let graph = create_graph();
        let config = vec![TestEntity {
            id: "a".to_string(),
            deps: vec![],
        }];

        let entity = config[0].clone();
        let chain = graph.resolve_chain(&config, &entity).unwrap();
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].id, "a");
    }

    #[test]
    fn test_resolve_chain_linear() {
        let graph = create_graph();
        let config = vec![
            TestEntity {
                id: "a".to_string(),
                deps: vec![],
            },
            TestEntity {
                id: "b".to_string(),
                deps: vec!["a".to_string()],
            },
            TestEntity {
                id: "c".to_string(),
                deps: vec!["b".to_string()],
            },
        ];

        let entity_c = config[2].clone();
        let chain = graph.resolve_chain(&config, &entity_c).unwrap();

        // Should include all dependencies in order: a, b, c
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].id, "a");
        assert_eq!(chain[1].id, "b");
        assert_eq!(chain[2].id, "c");
    }

    #[test]
    fn test_resolve_chain_circular() {
        let graph = create_graph();
        let config = vec![
            TestEntity {
                id: "a".to_string(),
                deps: vec!["b".to_string()],
            },
            TestEntity {
                id: "b".to_string(),
                deps: vec!["a".to_string()],
            },
        ];

        let entity_a = config[0].clone();
        let result = graph.resolve_chain(&config, &entity_a);
        assert!(matches!(result, Err(Error::CircularDependency { .. })));
    }

    #[test]
    fn test_topological_sort() {
        let graph = create_graph();
        let config = vec![
            TestEntity {
                id: "c".to_string(),
                deps: vec!["a".to_string(), "b".to_string()],
            },
            TestEntity {
                id: "b".to_string(),
                deps: vec!["a".to_string()],
            },
            TestEntity {
                id: "a".to_string(),
                deps: vec![],
            },
        ];

        let sorted = graph.topological_sort(&config, &config).unwrap();

        // Should be in dependency order: a, b, c
        assert_eq!(sorted[0].id, "a");
        assert_eq!(sorted[1].id, "b");
        assert_eq!(sorted[2].id, "c");
    }

    #[test]
    fn test_check_cycles_none() {
        let graph = create_graph();
        let config = vec![
            TestEntity {
                id: "a".to_string(),
                deps: vec![],
            },
            TestEntity {
                id: "b".to_string(),
                deps: vec!["a".to_string()],
            },
        ];

        assert!(graph.check_cycles(&config, &config).is_ok());
    }

    #[test]
    fn test_check_cycles_detected() {
        let graph = create_graph();
        let config = vec![
            TestEntity {
                id: "a".to_string(),
                deps: vec!["b".to_string()],
            },
            TestEntity {
                id: "b".to_string(),
                deps: vec!["a".to_string()],
            },
        ];

        assert!(matches!(
            graph.check_cycles(&config, &config),
            Err(Error::CircularDependency { .. })
        ));
    }
}

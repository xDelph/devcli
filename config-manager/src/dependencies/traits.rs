//! Core dependency provider trait.

/// Trait for extracting dependencies from entities.
///
/// Implementations define how to get the list of dependencies for a given entity.
///
/// # Type Parameters
///
/// * `E` - The entity type
///
/// # Example
///
/// ```rust
/// use config_manager::dependencies::DependencyProvider;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct App {
///     name: String,
///     depends_on: Vec<String>,
/// }
///
/// struct AppDependencyProvider;
///
/// impl DependencyProvider<App> for AppDependencyProvider {
///     fn dependencies(&self, entity: &App) -> Vec<String> {
///         entity.depends_on.clone()
///     }
/// }
/// ```
pub trait DependencyProvider<E>: Send + Sync {
    /// Get the direct dependencies of an entity.
    ///
    /// Returns a list of dependency identifiers (not the entities themselves).
    fn dependencies(&self, entity: &E) -> Vec<String>;

    /// Check if an entity has any dependencies.
    ///
    /// Default implementation checks if dependencies() is empty.
    fn has_dependencies(&self, entity: &E) -> bool {
        !self.dependencies(entity).is_empty()
    }

    /// Count the number of dependencies.
    ///
    /// Default implementation returns dependencies().len().
    fn dependency_count(&self, entity: &E) -> usize {
        self.dependencies(entity).len()
    }
}

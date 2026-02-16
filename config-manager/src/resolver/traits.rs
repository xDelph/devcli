//! Core resolver trait definition.

use crate::core::Result;

/// Trait for resolving entities by identifier.
///
/// Resolvers are responsible for finding entities within a configuration
/// structure, handling ambiguity, and providing helpful error messages.
///
/// # Type Parameters
///
/// * `C` - The configuration type to search within
/// * `E` - The entity type to resolve
///
/// # Example
///
/// ```rust
/// use config_manager::resolver::Resolver;
/// use std::collections::HashMap;
///
/// struct MyResolver;
///
/// impl Resolver<HashMap<String, String>, String> for MyResolver {
///     fn resolve(&self, config: &HashMap<String, String>, id: &str) -> config_manager::core::Result<String> {
///         config.get(id)
///             .cloned()
///             .ok_or_else(|| config_manager::core::Error::NotFound {
///                 id: id.to_string(),
///                 suggestion: None,
///             })
///     }
///
///     fn list_ids(&self, config: &HashMap<String, String>) -> Vec<String> {
///         config.keys().cloned().collect()
///     }
/// }
/// ```
pub trait Resolver<C, E>: Send + Sync {
    /// Resolve a single entity by identifier.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to search within
    /// * `id` - The identifier to resolve
    ///
    /// # Errors
    ///
    /// * `Error::NotFound` - Entity not found (may include suggestion)
    /// * `Error::Ambiguous` - Multiple entities match (should list candidates)
    fn resolve(&self, config: &C, id: &str) -> Result<E>;

    /// List all available entity identifiers.
    ///
    /// Used for validation and autocomplete functionality.
    fn list_ids(&self, config: &C) -> Vec<String>;

    /// Find similar identifiers for typo suggestions.
    ///
    /// Returns the closest match if one exists within a reasonable distance.
    ///
    /// Default implementation returns None (no suggestions).
    fn suggest(&self, _config: &C, _id: &str) -> Option<String> {
        None
    }

    /// Check if an identifier exists.
    ///
    /// Default implementation checks if id is in list_ids().
    fn exists(&self, config: &C, id: &str) -> bool {
        self.list_ids(config).iter().any(|i| i == id)
    }

    /// Resolve multiple entities by their identifiers.
    ///
    /// Default implementation calls resolve() for each id.
    fn resolve_many(&self, config: &C, ids: &[&str]) -> Result<Vec<E>> {
        ids.iter().map(|id| self.resolve(config, id)).collect()
    }
}

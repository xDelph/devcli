//! HashMap-based resolver implementation.

use crate::core::{Error, Result};
use crate::resolver::{fuzzy_match, Resolver};
use std::collections::HashMap;
use std::hash::Hash;

/// A simple resolver for HashMap-based configurations.
///
/// Resolves entities by key with optional fuzzy matching for typo suggestions.
///
/// # Examples
///
/// ```rust
/// use config_manager::resolver::{Resolver, HashMapResolver};
/// use std::collections::HashMap;
///
/// let mut config = HashMap::new();
/// config.insert("api".to_string(), "http://localhost:3000".to_string());
/// config.insert("db".to_string(), "postgres://localhost".to_string());
///
/// let resolver = HashMapResolver::new(true); // Enable fuzzy matching
///
/// let api_url = resolver.resolve(&config, "api").unwrap();
/// assert_eq!(api_url, "http://localhost:3000");
///
/// // Fuzzy matching for typos
/// let result = resolver.resolve(&config, "ap"); // Close to "api"
/// assert!(result.is_err()); // Error includes suggestion
/// ```
#[derive(Debug, Clone)]
pub struct HashMapResolver {
    fuzzy_matching: bool,
    max_distance: usize,
}

impl HashMapResolver {
    /// Create a new HashMap resolver.
    ///
    /// # Arguments
    ///
    /// * `fuzzy_matching` - Enable fuzzy matching for typo suggestions
    pub fn new(fuzzy_matching: bool) -> Self {
        Self {
            fuzzy_matching,
            max_distance: 2,
        }
    }

    /// Set the maximum Levenshtein distance for fuzzy matching.
    ///
    /// Default: 2
    pub fn with_max_distance(mut self, distance: usize) -> Self {
        self.max_distance = distance;
        self
    }
}

impl<K, V> Resolver<HashMap<K, V>, V> for HashMapResolver
where
    K: Eq + Hash + AsRef<str> + ToString + Send + Sync,
    V: Clone + Send + Sync,
{
    fn resolve(&self, config: &HashMap<K, V>, id: &str) -> Result<V> {
        // Try direct lookup first
        for (key, value) in config {
            if key.as_ref() == id {
                return Ok(value.clone());
            }
        }

        // Not found - try fuzzy matching for suggestions
        if self.fuzzy_matching {
            let ids = self.list_ids(config);
            if let Some(suggestion) = fuzzy_match(id, &ids, self.max_distance) {
                return Err(Error::NotFound {
                    id: id.to_string(),
                    suggestion: Some(suggestion),
                });
            }
        }

        Err(Error::NotFound {
            id: id.to_string(),
            suggestion: None,
        })
    }

    fn list_ids(&self, config: &HashMap<K, V>) -> Vec<String> {
        config.keys().map(|k| k.to_string()).collect()
    }

    fn suggest(&self, config: &HashMap<K, V>, id: &str) -> Option<String> {
        if self.fuzzy_matching {
            let ids = self.list_ids(config);
            fuzzy_match(id, &ids, self.max_distance)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hashmap_resolver_exact_match() {
        let mut config = HashMap::new();
        config.insert("redis", "6379");
        config.insert("postgres", "5432");

        let resolver = HashMapResolver::new(false);
        let result = resolver.resolve(&config, "redis").unwrap();
        assert_eq!(result, "6379");
    }

    #[test]
    fn test_hashmap_resolver_not_found() {
        let mut config = HashMap::new();
        config.insert("redis", "6379");

        let resolver = HashMapResolver::new(false);
        let result = resolver.resolve(&config, "mysql");
        assert!(matches!(result, Err(Error::NotFound { .. })));
    }

    #[test]
    fn test_hashmap_resolver_fuzzy_match() {
        let mut config = HashMap::new();
        config.insert("redis".to_string(), "6379".to_string());
        config.insert("postgres".to_string(), "5432".to_string());

        let resolver = HashMapResolver::new(true);
        let result = resolver.resolve(&config, "redi");

        match result {
            Err(Error::NotFound { suggestion, .. }) => {
                assert_eq!(suggestion, Some("redis".to_string()));
            }
            _ => panic!("Expected NotFound error with suggestion"),
        }
    }

    #[test]
    fn test_hashmap_resolver_list_ids() {
        let mut config = HashMap::new();
        config.insert("redis", "6379");
        config.insert("postgres", "5432");

        let resolver = HashMapResolver::new(false);
        let mut ids = resolver.list_ids(&config);
        ids.sort();

        assert_eq!(ids, vec!["postgres", "redis"]);
    }

    #[test]
    fn test_hashmap_resolver_suggest() {
        let mut config = HashMap::new();
        config.insert("redis".to_string(), "6379".to_string());

        let resolver = HashMapResolver::new(true);
        let suggestion = resolver.suggest(&config, "redi");
        assert_eq!(suggestion, Some("redis".to_string()));

        let suggestion = resolver.suggest(&config, "mysql");
        assert_eq!(suggestion, None);
    }
}

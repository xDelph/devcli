//! Detection context with caching and lazy loading

use crate::{types::DetectionResult, Result};
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// The type of detection scope (root project vs workspace within monorepo)
#[derive(Debug, Clone, PartialEq)]
pub enum ScopeType {
    /// Top-level project detection
    Root,
    /// A workspace within a monorepo (e.g., apps/web inside an Nx repo)
    Workspace {
        monorepo_root: PathBuf,
        workspace_name: String,
    },
}

/// Tracks recursion depth and cycle detection for hierarchical detection
#[derive(Debug, Clone)]
pub struct DetectionScope {
    /// Current recursion depth (0 = root)
    pub depth: usize,
    /// Maximum allowed depth
    pub max_depth: usize,
    /// Chain of ancestor paths for cycle detection
    pub parent_paths: Vec<PathBuf>,
    /// What kind of detection scope this is
    pub scope_type: ScopeType,
}

impl DetectionScope {
    /// Create a root-level scope
    pub fn root(max_depth: usize) -> Self {
        Self {
            depth: 0,
            max_depth,
            parent_paths: Vec::new(),
            scope_type: ScopeType::Root,
        }
    }

    /// Create a child workspace scope from a parent scope
    pub fn workspace(parent: &DetectionScope, workspace_name: String, monorepo_root: PathBuf) -> Self {
        let mut parent_paths = parent.parent_paths.clone();
        parent_paths.push(monorepo_root.clone());
        Self {
            depth: parent.depth + 1,
            max_depth: parent.max_depth,
            parent_paths,
            scope_type: ScopeType::Workspace { monorepo_root, workspace_name },
        }
    }

    /// True if we can recurse into `target` (depth limit not reached + no cycle)
    pub fn can_recurse(&self, target: &Path) -> bool {
        if self.depth >= self.max_depth {
            return false;
        }
        // Cycle detection: target must not be an ancestor path
        !self.parent_paths.iter().any(|p| p == target)
    }

    /// True if this scope is inside a monorepo workspace
    pub fn is_workspace(&self) -> bool {
        matches!(self.scope_type, ScopeType::Workspace { .. })
    }
}

/// Immutable context passed to all detection strategies
///
/// Contains all information about the directory being analyzed with
/// lazy loading and caching for performance.
pub struct DetectionContext {
    /// Absolute path to the project root
    pub root_path: PathBuf,

    /// Scope information (depth, parent paths, workspace type)
    pub scope: DetectionScope,

    /// Lazy-loaded file tree (cached)
    files: OnceCell<FileTree>,

    /// File content cache
    content_cache: Arc<RwLock<HashMap<PathBuf, String>>>,

    /// Results from previously executed strategies
    previous_results: Arc<RwLock<HashMap<String, DetectionResult>>>,

    /// Maximum depth for directory traversal
    max_depth: usize,

    /// Patterns to ignore
    ignore_patterns: Vec<String>,
}

impl DetectionContext {
    /// Create a new detection context (root scope, default depth limit of 2)
    pub fn new(root_path: impl AsRef<Path>) -> Result<Self> {
        Self::with_scope(root_path, DetectionScope::root(2))
    }

    /// Create a detection context with an explicit scope
    pub fn with_scope(root_path: impl AsRef<Path>, scope: DetectionScope) -> Result<Self> {
        let root_path = root_path.as_ref().to_path_buf();

        if !root_path.exists() {
            anyhow::bail!("Path does not exist: {}", root_path.display());
        }

        if !root_path.is_dir() {
            anyhow::bail!("Path is not a directory: {}", root_path.display());
        }

        Ok(Self {
            root_path,
            scope,
            files: OnceCell::new(),
            content_cache: Arc::new(RwLock::new(HashMap::new())),
            previous_results: Arc::new(RwLock::new(HashMap::new())),
            max_depth: 3,
            ignore_patterns: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".venv".to_string(),
                "__pycache__".to_string(),
            ],
        })
    }

    /// True if this context is running inside a monorepo workspace
    pub fn is_workspace(&self) -> bool {
        self.scope.is_workspace()
    }

    /// Check if a file exists (cached via file tree)
    pub fn file_exists(&self, path: impl AsRef<Path>) -> bool {
        let full_path = self.root_path.join(path.as_ref());
        full_path.exists()
    }

    /// Read file content (cached)
    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<String> {
        let full_path = self.root_path.join(path.as_ref());

        // Check cache first
        {
            let cache = self.content_cache.read().unwrap();
            if let Some(content) = cache.get(&full_path) {
                return Ok(content.clone());
            }
        }

        // Read from disk
        let content = fs::read_to_string(&full_path)?;

        // Store in cache
        {
            let mut cache = self.content_cache.write().unwrap();
            cache.insert(full_path, content.clone());
        }

        Ok(content)
    }

    /// Parse JSON file (cached + parsed)
    pub fn parse_json<T: serde::de::DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<T> {
        let content = self.read_file(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Parse TOML file (cached + parsed)
    pub fn parse_toml<T: serde::de::DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<T> {
        let content = self.read_file(path)?;
        Ok(toml::from_str(&content)?)
    }

    /// Get result from a previously executed strategy
    pub fn get_result(&self, strategy_id: &str) -> Option<DetectionResult> {
        let results = self.previous_results.read().unwrap();
        results.get(strategy_id).cloned()
    }

    /// Store a result from a strategy (internal use by engine)
    pub(crate) fn store_result(&self, result: DetectionResult) {
        let mut results = self.previous_results.write().unwrap();
        results.insert(result.strategy_id.clone(), result);
    }

    /// Get the file tree (lazy loaded)
    fn get_file_tree(&self) -> &FileTree {
        self.files.get_or_init(|| {
            FileTree::build(&self.root_path, self.max_depth, &self.ignore_patterns)
                .unwrap_or_else(|_| FileTree::empty())
        })
    }

    /// List all files in the project
    pub fn list_files(&self) -> Vec<PathBuf> {
        self.get_file_tree().files.clone()
    }

    /// Find files matching a pattern (glob-like)
    pub fn glob(&self, pattern: &str) -> Vec<PathBuf> {
        let tree = self.get_file_tree();

        // Simple pattern matching for now
        // TODO: Implement full glob support
        tree.files
            .iter()
            .filter(|path| {
                let path_str = path.to_string_lossy();

                // Handle different patterns
                if pattern.starts_with("**/") {
                    // Recursive pattern: **/*.rs matches any .rs file at any depth
                    let suffix = pattern.trim_start_matches("**/");
                    if suffix.starts_with("*.") {
                        // Pattern like **/*.rs
                        let ext = suffix.trim_start_matches("*.");
                        path_str.ends_with(&format!(".{}", ext))
                    } else if suffix.ends_with('*') {
                        // Pattern like **/Dockerfile* - match files starting with prefix
                        let prefix = suffix.trim_end_matches('*');
                        path_str.split('/').any(|part| part.starts_with(prefix))
                    } else {
                        // Pattern like **/Dockerfile - exact filename match
                        path_str.split('/').any(|part| part == suffix)
                    }
                } else if pattern.starts_with("*.") {
                    // Extension match: *.rs — root-level files only (no subdirectory separator)
                    // Use **/*.rs for recursive matching across subdirectories
                    let ext = pattern.trim_start_matches("*.");
                    !path_str.contains('/') && path_str.ends_with(&format!(".{}", ext))
                } else if pattern.ends_with("/*") {
                    // Directory match: src/*
                    let prefix = pattern.trim_end_matches("/*");
                    path_str.starts_with(&format!("{}/", prefix))
                        && !path_str[prefix.len() + 1..].contains('/')
                } else {
                    // Exact or contains match
                    path_str.contains(pattern)
                }
            })
            .cloned()
            .collect()
    }
}

/// Lazy file tree for efficient directory traversal
struct FileTree {
    /// All files relative to root
    files: Vec<PathBuf>,
    /// All directories relative to root
    #[allow(dead_code)]
    directories: Vec<PathBuf>,
}

impl FileTree {
    fn empty() -> Self {
        Self {
            files: Vec::new(),
            directories: Vec::new(),
        }
    }

    fn build(root: &Path, max_depth: usize, ignore_patterns: &[String]) -> Result<Self> {
        let mut files = Vec::new();
        let mut directories = Vec::new();

        Self::scan_dir(root, root, 0, max_depth, ignore_patterns, &mut files, &mut directories)?;

        Ok(Self { files, directories })
    }

    fn scan_dir(
        root: &Path,
        current: &Path,
        depth: usize,
        max_depth: usize,
        ignore_patterns: &[String],
        files: &mut Vec<PathBuf>,
        directories: &mut Vec<PathBuf>,
    ) -> Result<()> {
        if depth > max_depth {
            return Ok(());
        }

        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();

            // Check ignore patterns
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if ignore_patterns.iter().any(|pattern| name.contains(pattern)) {
                    continue;
                }
            }

            if path.is_dir() {
                if let Ok(rel_path) = path.strip_prefix(root) {
                    directories.push(rel_path.to_path_buf());
                }
                Self::scan_dir(root, &path, depth + 1, max_depth, ignore_patterns, files, directories)?;
            } else if path.is_file() {
                if let Ok(rel_path) = path.strip_prefix(root) {
                    files.push(rel_path.to_path_buf());
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_context_new() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        assert_eq!(ctx.root_path, temp_dir.path());
    }

    #[test]
    fn test_file_exists() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        File::create(&test_file).unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        assert!(ctx.file_exists("test.txt"));
        assert!(!ctx.file_exists("nonexistent.txt"));
    }

    #[test]
    fn test_read_file_cached() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"Hello, World!").unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();

        // First read
        let content1 = ctx.read_file("test.txt").unwrap();
        assert_eq!(content1, "Hello, World!");

        // Second read (should hit cache)
        let content2 = ctx.read_file("test.txt").unwrap();
        assert_eq!(content2, "Hello, World!");
    }

    #[test]
    fn test_parse_json() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.json");
        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"{\"name\":\"test\",\"version\":\"1.0\"}").unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();

        #[derive(serde::Deserialize)]
        struct TestData {
            name: String,
            version: String,
        }

        let data: TestData = ctx.parse_json("test.json").unwrap();
        assert_eq!(data.name, "test");
        assert_eq!(data.version, "1.0");
    }

    #[test]
    fn test_glob_wildcard_pattern() {
        let temp_dir = TempDir::new().unwrap();

        // Create docker directory with Dockerfile variants
        std::fs::create_dir(temp_dir.path().join("docker")).unwrap();
        File::create(temp_dir.path().join("docker/Dockerfile")).unwrap();
        File::create(temp_dir.path().join("docker/Dockerfile.dev")).unwrap();
        File::create(temp_dir.path().join("Dockerfile.prod")).unwrap();
        File::create(temp_dir.path().join("README.md")).unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();

        // Test **/Dockerfile* pattern
        let results = ctx.glob("**/Dockerfile*");
        assert_eq!(results.len(), 3, "Should find all Dockerfile variants");

        let result_strs: Vec<String> = results.iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        assert!(result_strs.iter().any(|s| s.contains("docker/Dockerfile")));
        assert!(result_strs.iter().any(|s| s.contains("Dockerfile.dev")));
        assert!(result_strs.iter().any(|s| s.contains("Dockerfile.prod")));
    }
}

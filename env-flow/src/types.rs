use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Deployment stage (dev / qa / prod / …)
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Stage {
    Dev,
    Qa,
    Staging,
    Preprod,
    Prod,
    Test,
    Custom(String),
}

impl Stage {
    pub fn as_str(&self) -> &str {
        match self {
            Stage::Dev => "dev",
            Stage::Qa => "qa",
            Stage::Staging => "staging",
            Stage::Preprod => "preprod",
            Stage::Prod => "prod",
            Stage::Test => "test",
            Stage::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for Stage {
    fn from(s: &str) -> Self {
        match s {
            "dev" | "development" => Stage::Dev,
            "qa" => Stage::Qa,
            "staging" => Stage::Staging,
            "preprod" | "pre-prod" | "pre_prod" => Stage::Preprod,
            "prod" | "production" => Stage::Prod,
            "test" => Stage::Test,
            other => Stage::Custom(other.to_string()),
        }
    }
}

impl From<String> for Stage {
    fn from(s: String) -> Self {
        Stage::from(s.as_str())
    }
}

// ─── RuntimeContext ───────────────────────────────────────────────────────────

/// Where the process is running — determines which env layers are loaded.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuntimeContext {
    /// Developer's local machine (default)
    Local,
    /// Plain Docker (`docker run` / `docker build`)
    Docker,
    /// Docker Compose (`COMPOSE_PROJECT_NAME` is set)
    DockerCompose,
    /// Kubernetes (`KUBERNETES_SERVICE_HOST` is set)
    Kubernetes,
    /// OrbStack (macOS container runtime, detected via `/.dockerenv` + OrbStack marker)
    OrbStack,
    /// Generic CI system (GitHub Actions, GitLab CI, CircleCI, Jenkins, …)
    CI,
    /// Any other named context
    Custom(String),
}

impl RuntimeContext {
    /// True for contexts that should NOT load `.env.local` / `.env.{stage}.local`.
    /// Containers and CI servers have no concept of a local developer machine.
    pub fn skip_local_files(&self) -> bool {
        matches!(
            self,
            RuntimeContext::Docker
                | RuntimeContext::DockerCompose
                | RuntimeContext::Kubernetes
                | RuntimeContext::OrbStack
                | RuntimeContext::CI
        )
    }

    /// True for container runtimes (Docker, DockerCompose, Kubernetes, OrbStack).
    pub fn is_container(&self) -> bool {
        matches!(
            self,
            RuntimeContext::Docker
                | RuntimeContext::DockerCompose
                | RuntimeContext::Kubernetes
                | RuntimeContext::OrbStack
        )
    }

    /// The string identifier used in file names (e.g. `.env.docker`, `docker/.env`).
    pub fn as_str(&self) -> &str {
        match self {
            RuntimeContext::Local => "local",
            RuntimeContext::Docker => "docker",
            RuntimeContext::DockerCompose => "docker",
            RuntimeContext::Kubernetes => "k8s",
            RuntimeContext::OrbStack => "orbstack",
            RuntimeContext::CI => "ci",
            RuntimeContext::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for RuntimeContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for RuntimeContext {
    fn from(s: &str) -> Self {
        match s {
            "local" => RuntimeContext::Local,
            "docker" => RuntimeContext::Docker,
            "docker-compose" | "compose" => RuntimeContext::DockerCompose,
            "k8s" | "kubernetes" => RuntimeContext::Kubernetes,
            "orbstack" => RuntimeContext::OrbStack,
            "ci" => RuntimeContext::CI,
            other => RuntimeContext::Custom(other.to_string()),
        }
    }
}

impl From<String> for RuntimeContext {
    fn from(s: String) -> Self {
        RuntimeContext::from(s.as_str())
    }
}

// ─── EnvEntry & EnvVars ──────────────────────────────────────────────────────

/// A single resolved variable — value plus audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvEntry {
    pub value: String,
    /// The file that "won" for this key (last writer wins in layer order).
    pub source_file: PathBuf,
    /// 1-based line number in `source_file`.
    pub source_line: usize,
    /// Earlier files that also set this key (overridden values).
    pub overridden_by: Vec<PathBuf>,
    /// Whether `${VAR}` interpolation should be applied.
    /// False for single-quoted values (bash semantics).
    pub interpolate: bool,
}

/// The result of loading env layers: an ordered map of key → `EnvEntry`.
///
/// Uses `IndexMap` so iteration order matches insertion order (layer order).
#[derive(Debug, Default, Clone)]
pub struct EnvVars(pub(crate) indexmap::IndexMap<String, EnvEntry>);

impl EnvVars {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|e| e.value.as_str())
    }

    pub fn get_parsed<T>(&self, key: &str) -> crate::Result<T>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        let raw = self
            .get(key)
            .ok_or_else(|| crate::Error::MissingVariable(key.to_string()))?;
        raw.parse::<T>()
            .map_err(|e| crate::Error::ParseValue(key.to_string(), e.to_string()))
    }

    pub fn get_with_source(&self, key: &str) -> Option<(&str, &std::path::Path)> {
        self.0
            .get(key)
            .map(|e| (e.value.as_str(), e.source_file.as_path()))
    }

    /// Returns an error listing every missing key.
    pub fn require(&self, keys: &[&str]) -> crate::Result<()> {
        let missing: Vec<String> = keys
            .iter()
            .filter(|k| !self.0.contains_key(**k))
            .map(|k| k.to_string())
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(crate::Error::RequiredMissing(missing))
        }
    }

    /// Set all variables in `std::env`.
    pub fn apply(&self) {
        for (key, entry) in &self.0 {
            std::env::set_var(key, &entry.value);
        }
    }

    /// Iterate over (key, value) pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, e)| (k.as_str(), e.value.as_str()))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Merge `other` into this map; keys from `other` override existing entries.
    pub fn merge_from(&mut self, other: Self) {
        for (key, entry) in other.0 {
            self.0.insert(key, entry);
        }
    }

    /// Serialize resolved variables to dotenv format (for temp `--env-file` files).
    pub fn to_dotenv(&self) -> String {
        let mut out = String::new();
        for (key, entry) in &self.0 {
            out.push_str(key);
            out.push('=');
            out.push_str(&format_dotenv_value(&entry.value));
            out.push('\n');
        }
        out
    }

    /// Write resolved variables to a dotenv file.
    pub fn write_dotenv(&self, path: &std::path::Path) -> crate::Result<()> {
        std::fs::write(path, self.to_dotenv()).map_err(|source| crate::Error::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}

fn format_dotenv_value(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value.contains(' ')
        || value.contains('\t')
        || value.contains('\n')
        || value.contains('"')
        || value.contains('#')
        || value.contains('$');

    if needs_quotes {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

// ─── Layer types ─────────────────────────────────────────────────────────────

/// Which layer in the 2D model a file belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerType {
    /// `.env` — always-loaded base
    Base,
    /// `.env.{context}` or `{context}/.env`
    ContextBase,
    /// `.env.{stage}`
    StageBase,
    /// `.env.{stage}.{context}` or `{context}/.env.{stage}`
    StageContext,
    /// `.env.local` — local machine override (skipped in containers/CI)
    LocalOverride,
    /// `.env.{stage}.local` — local machine + stage override
    StageLocalOverride,
}

/// A single resolved layer path — may or may not exist on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedLayer {
    pub path: PathBuf,
    pub relative_path: String,
    /// `false` → the file would be loaded if it existed; used for transparency.
    pub exists: bool,
    pub layer_type: LayerType,
}

// ─── Parsed entry (internal, from parser) ────────────────────────────────────

/// Internal representation of a single line from a `.env` file.
#[derive(Debug, Clone)]
pub struct ParsedEntry {
    pub key: String,
    pub raw_value: String,
    /// Line number (1-based) in source file.
    pub line_number: usize,
    /// False for single-quoted values — interpolation must be skipped.
    pub interpolate: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_from_str() {
        assert_eq!(Stage::from("dev"), Stage::Dev);
        assert_eq!(Stage::from("development"), Stage::Dev);
        assert_eq!(Stage::from("production"), Stage::Prod);
        assert_eq!(Stage::from("prod"), Stage::Prod);
        assert_eq!(Stage::from("staging"), Stage::Staging);
        assert_eq!(Stage::from("preprod"), Stage::Preprod);
        assert_eq!(Stage::from("test"), Stage::Test);
        assert_eq!(Stage::from("qa"), Stage::Qa);
        assert_eq!(Stage::from("nightly"), Stage::Custom("nightly".to_string()));
    }

    #[test]
    fn stage_display() {
        assert_eq!(Stage::Dev.to_string(), "dev");
        assert_eq!(Stage::Prod.to_string(), "prod");
        assert_eq!(Stage::Custom("nightly".to_string()).to_string(), "nightly");
    }

    #[test]
    fn context_skip_local_files() {
        assert!(!RuntimeContext::Local.skip_local_files());
        assert!(RuntimeContext::Docker.skip_local_files());
        assert!(RuntimeContext::DockerCompose.skip_local_files());
        assert!(RuntimeContext::Kubernetes.skip_local_files());
        assert!(RuntimeContext::OrbStack.skip_local_files());
        assert!(RuntimeContext::CI.skip_local_files());
    }

    #[test]
    fn context_is_container() {
        assert!(!RuntimeContext::Local.is_container());
        assert!(!RuntimeContext::CI.is_container());
        assert!(RuntimeContext::Docker.is_container());
        assert!(RuntimeContext::Kubernetes.is_container());
        assert!(RuntimeContext::OrbStack.is_container());
    }

    #[test]
    fn context_as_str_docker_compose() {
        // DockerCompose uses "docker" as file-name key
        assert_eq!(RuntimeContext::DockerCompose.as_str(), "docker");
    }
}

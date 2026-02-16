//! Core types for detection results and data structures

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// App type categories - WHAT the application is
/// Phase 1 detection: Identifies the core nature of the application
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppTypeCategory {
    /// Base language/runtime (Node.js, Python, Rust, Go, Java, etc.)
    Language,

    /// Framework built on a language (React, Django, Rails, Spring, etc.)
    Framework,

    /// Monorepo tool (Nx, Turborepo, Lerna, Bazel, etc.)
    Monorepo,

    /// Build tool (webpack, vite, gradle, maven, cargo, etc.)
    BuildTool,

    /// Package manager (npm, yarn, pnpm, pip, cargo, go mod, etc.)
    PackageManager,

    /// Database (PostgreSQL, Redis, MongoDB, etc.)
    Database,

    /// Service/Infrastructure (Redis, Traefik, Nginx, etc.)
    Service,

    /// Message queue (RabbitMQ, Kafka, NATS, etc.)
    MessageQueue,

    /// Reverse proxy (Nginx, Traefik, Caddy, HAProxy, etc.)
    ReverseProxy,

    /// CI/CD (GitHub Actions, GitLab CI, Jenkins, etc.)
    Cicd,

    /// Testing framework (Jest, pytest, JUnit, etc.)
    Testing,

    /// Linter/formatter (ESLint, Prettier, Black, rustfmt, etc.)
    Linting,

    /// Custom/other
    Custom(String),
}

/// Environment capability categories - HOW the application can be run
/// Phase 2 detection: Identifies available deployment/runtime environments
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvCapabilityCategory {
    /// Local development environment (npm scripts, python commands, etc.)
    Local,

    /// Docker container environment
    Docker,

    /// OrbStack container environment
    OrbStack,

    /// Kubernetes orchestration
    Kubernetes,

    /// Docker Swarm orchestration
    DockerSwarm,

    /// Nomad orchestration
    Nomad,

    /// Cloud platform (AWS, GCP, Azure, Vercel, etc.)
    CloudPlatform,

    /// IaC tool (Terraform, Pulumi, CloudFormation, etc.)
    Infrastructure,

    /// Custom environment
    Custom(String),
}

/// Unified category enum for backward compatibility
/// Allows strategies to specify either app type or environment capability
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum StrategyCategory {
    AppType(AppTypeCategory),
    EnvCapability(EnvCapabilityCategory),
}

impl StrategyCategory {
    /// Check if this is an app type category
    pub fn is_app_type(&self) -> bool {
        matches!(self, StrategyCategory::AppType(_))
    }

    /// Check if this is an environment capability category
    pub fn is_env_capability(&self) -> bool {
        matches!(self, StrategyCategory::EnvCapability(_))
    }

    /// Get the app type category if applicable
    pub fn as_app_type(&self) -> Option<&AppTypeCategory> {
        match self {
            StrategyCategory::AppType(cat) => Some(cat),
            _ => None,
        }
    }

    /// Get the env capability category if applicable
    pub fn as_env_capability(&self) -> Option<&EnvCapabilityCategory> {
        match self {
            StrategyCategory::EnvCapability(cat) => Some(cat),
            _ => None,
        }
    }
}

/// Result from a detection strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    /// Strategy that produced this result
    pub strategy_id: String,

    /// Category of this detection
    pub category: StrategyCategory,

    /// Confidence level (0.0 - 1.0)
    /// 1.0 = definitive, 0.5 = likely, 0.0 = uncertain
    pub confidence: f32,

    /// Structured data from detection
    pub data: DetectionData,

    /// Optional: Suggested next strategies to run
    #[serde(default)]
    pub suggested_strategies: Vec<String>,
}

/// Flexible data structure for detection results
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DetectionData {
    /// Simple key-value pairs
    KeyValue(HashMap<String, serde_json::Value>),

    // ===== App Type Data =====

    /// Typed language detection
    Language(LanguageInfo),

    /// Typed framework detection
    Framework(FrameworkInfo),

    /// Typed monorepo detection
    Monorepo(MonorepoInfo),

    /// Typed database/service detection
    Service(ServiceInfo),

    /// Typed package manager detection
    PackageManager(PackageManagerInfo),

    // ===== Environment Capability Data =====

    /// Local environment commands
    LocalEnv(LocalEnvInfo),

    /// Docker container environment
    DockerEnv(DockerEnvInfo),

    /// OrbStack container environment
    OrbStackEnv(OrbStackEnvInfo),

    /// Kubernetes environment
    KubernetesEnv(KubernetesEnvInfo),

    /// Custom structured data
    Custom(serde_json::Value),
}

/// Language detection info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub name: String,
    pub version: Option<String>,
    pub version_source: Option<String>,
    pub primary_files: Vec<PathBuf>,
    #[serde(default)]
    pub total_lines: Option<usize>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Framework detection info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkInfo {
    pub name: String,
    pub version: Option<String>,
    pub based_on_language: String,
    pub config_files: Vec<PathBuf>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Container detection info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub platform: String,
    pub dockerfiles: Vec<PathBuf>,
    #[serde(default)]
    pub compose_files: Vec<PathBuf>,
    #[serde(default)]
    pub stages: Vec<String>,
    #[serde(default)]
    pub base_images: Vec<String>,
    #[serde(default)]
    pub exposed_ports: Vec<u16>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Package manager detection info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManagerInfo {
    pub name: String,
    pub version: Option<String>,
    pub lock_file: Option<PathBuf>,
    pub manifest_file: Option<PathBuf>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Monorepo detection info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonorepoInfo {
    pub tool: String,
    pub version: Option<String>,
    pub config_file: PathBuf,
    #[serde(default)]
    pub workspaces: Vec<String>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Service detection info (Redis, Traefik, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub version: Option<String>,
    pub config_files: Vec<PathBuf>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Local environment info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalEnvInfo {
    /// Available commands (e.g., "start" -> "npm start")
    pub commands: HashMap<String, String>,
    /// Suggested default command
    pub suggested_default: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Docker environment info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerEnvInfo {
    pub dockerfiles: Vec<PathBuf>,
    #[serde(default)]
    pub compose_files: Vec<PathBuf>,
    #[serde(default)]
    pub stages: Vec<String>,
    #[serde(default)]
    pub base_images: Vec<String>,
    #[serde(default)]
    pub exposed_ports: Vec<u16>,
    /// Available commands (e.g., "start" -> "docker compose up")
    #[serde(default)]
    pub commands: HashMap<String, String>,
    /// Suggested default command
    pub suggested_default: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// OrbStack environment info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbStackEnvInfo {
    /// Available commands
    pub commands: HashMap<String, String>,
    /// Suggested default command
    pub suggested_default: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Kubernetes environment info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesEnvInfo {
    pub manifests: Vec<PathBuf>,
    #[serde(default)]
    pub helm_charts: Vec<PathBuf>,
    /// Available commands (e.g., "apply" -> "kubectl apply -f ...")
    #[serde(default)]
    pub commands: HashMap<String, String>,
    /// Suggested default command
    pub suggested_default: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Detection report containing all results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionReport {
    pub path: PathBuf,
    pub results: Vec<DetectionResult>,
}

impl DetectionReport {
    /// Create a new empty report
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            results: Vec::new(),
        }
    }

    /// Add a detection result
    pub fn add_result(&mut self, result: DetectionResult) {
        self.results.push(result);
    }

    /// Get all results by category
    pub fn by_category(&self, category: &StrategyCategory) -> Vec<&DetectionResult> {
        self.results
            .iter()
            .filter(|r| &r.category == category)
            .collect()
    }

    /// Get all results by app type category
    pub fn by_app_type(&self, app_type: &AppTypeCategory) -> Vec<&DetectionResult> {
        self.results
            .iter()
            .filter(|r| r.category.as_app_type() == Some(app_type))
            .collect()
    }

    /// Get all results by environment capability category
    pub fn by_env_capability(&self, env_cap: &EnvCapabilityCategory) -> Vec<&DetectionResult> {
        self.results
            .iter()
            .filter(|r| r.category.as_env_capability() == Some(env_cap))
            .collect()
    }

    /// Get all app type detections
    pub fn app_types(&self) -> Vec<&DetectionResult> {
        self.results
            .iter()
            .filter(|r| r.category.is_app_type())
            .collect()
    }

    /// Get all environment capability detections
    pub fn env_capabilities(&self) -> Vec<&DetectionResult> {
        self.results
            .iter()
            .filter(|r| r.category.is_env_capability())
            .collect()
    }

    /// Get primary language (highest confidence)
    pub fn primary_language(&self) -> Option<&DetectionResult> {
        self.by_app_type(&AppTypeCategory::Language)
            .into_iter()
            .max_by(|a, b| {
                a.confidence
                    .partial_cmp(&b.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get all frameworks detected
    pub fn frameworks(&self) -> Vec<&DetectionResult> {
        self.by_app_type(&AppTypeCategory::Framework)
    }

    /// Get all languages detected
    pub fn languages(&self) -> Vec<&DetectionResult> {
        self.by_app_type(&AppTypeCategory::Language)
    }

    /// Get all monorepos detected
    pub fn monorepos(&self) -> Vec<&DetectionResult> {
        self.by_app_type(&AppTypeCategory::Monorepo)
    }

    /// Get all services detected
    pub fn services(&self) -> Vec<&DetectionResult> {
        self.by_app_type(&AppTypeCategory::Service)
    }

    /// Check if a specific technology was detected
    pub fn has(&self, strategy_id: &str) -> bool {
        self.results.iter().any(|r| r.strategy_id == strategy_id)
    }

    /// Get result by strategy ID
    pub fn get(&self, strategy_id: &str) -> Option<&DetectionResult> {
        self.results.iter().find(|r| r.strategy_id == strategy_id)
    }

    /// Extract the application name from detection results
    ///
    /// Searches for `package_name` in metadata across all results, prioritizing:
    /// 1. Monorepo tools (highest priority - they define the workspace)
    /// 2. Languages (medium priority - they define the app)
    /// 3. Services (lowest priority - fallback)
    ///
    /// # Example
    ///
    /// ```
    /// use app_detector::{DetectionReport, DetectionResult, StrategyCategory, DetectionData};
    /// use app_detector::types::{AppTypeCategory, LanguageInfo};
    /// use std::collections::HashMap;
    /// use std::path::PathBuf;
    ///
    /// let mut report = DetectionReport::new(PathBuf::from("/test"));
    /// let mut metadata = HashMap::new();
    /// metadata.insert("package_name".to_string(), serde_json::json!("my-app"));
    ///
    /// report.add_result(DetectionResult {
    ///     strategy_id: "nodejs".to_string(),
    ///     category: StrategyCategory::AppType(AppTypeCategory::Language),
    ///     confidence: 1.0,
    ///     data: DetectionData::Language(LanguageInfo {
    ///         name: "Node.js".to_string(),
    ///         version: None,
    ///         version_source: None,
    ///         primary_files: vec![],
    ///         total_lines: None,
    ///         metadata,
    ///     }),
    ///     suggested_strategies: vec![],
    /// });
    ///
    /// assert_eq!(report.app_name(), Some("my-app".to_string()));
    /// ```
    pub fn app_name(&self) -> Option<String> {
        // Priority 1: Check monorepo tools
        for result in self.monorepos() {
            if let Some(name) = self.extract_package_name(result) {
                return Some(name);
            }
        }

        // Priority 2: Check languages
        for result in self.languages() {
            if let Some(name) = self.extract_package_name(result) {
                return Some(name);
            }
        }

        // Priority 3: Check services
        for result in self.services() {
            if let Some(name) = self.extract_package_name(result) {
                return Some(name);
            }
        }

        None
    }

    /// Helper to extract package_name from a result's metadata
    fn extract_package_name(&self, result: &DetectionResult) -> Option<String> {
        match &result.data {
            DetectionData::Language(info) => info.metadata.get("package_name"),
            DetectionData::Monorepo(info) => info.metadata.get("package_name"),
            DetectionData::Service(info) => info.metadata.get("package_name"),
            DetectionData::Framework(info) => info.metadata.get("package_name"),
            DetectionData::PackageManager(info) => info.metadata.get("package_name"),
            DetectionData::KeyValue(map) => map.get("package_name"),
            _ => None,
        }
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_report_new() {
        let report = DetectionReport::new(PathBuf::from("/test"));
        assert_eq!(report.results.len(), 0);
    }

    #[test]
    fn test_add_result() {
        let mut report = DetectionReport::new(PathBuf::from("/test"));

        let result = DetectionResult {
            strategy_id: "rust".to_string(),
            category: StrategyCategory::AppType(AppTypeCategory::Language),
            confidence: 1.0,
            data: DetectionData::Language(LanguageInfo {
                name: "Rust".to_string(),
                version: Some("1.75.0".to_string()),
                version_source: None,
                primary_files: vec![],
                total_lines: None,
                metadata: HashMap::new(),
            }),
            suggested_strategies: vec![],
        };

        report.add_result(result);
        assert_eq!(report.results.len(), 1);
    }

    #[test]
    fn test_by_category() {
        let mut report = DetectionReport::new(PathBuf::from("/test"));

        report.add_result(DetectionResult {
            strategy_id: "rust".to_string(),
            category: StrategyCategory::AppType(AppTypeCategory::Language),
            confidence: 1.0,
            data: DetectionData::KeyValue(HashMap::new()),
            suggested_strategies: vec![],
        });

        report.add_result(DetectionResult {
            strategy_id: "docker".to_string(),
            category: StrategyCategory::EnvCapability(EnvCapabilityCategory::Docker),
            confidence: 1.0,
            data: DetectionData::KeyValue(HashMap::new()),
            suggested_strategies: vec![],
        });

        let languages = report.by_app_type(&AppTypeCategory::Language);
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].strategy_id, "rust");

        let docker_envs = report.by_env_capability(&EnvCapabilityCategory::Docker);
        assert_eq!(docker_envs.len(), 1);
        assert_eq!(docker_envs[0].strategy_id, "docker");
    }

    #[test]
    fn test_has() {
        let mut report = DetectionReport::new(PathBuf::from("/test"));

        report.add_result(DetectionResult {
            strategy_id: "rust".to_string(),
            category: StrategyCategory::AppType(AppTypeCategory::Language),
            confidence: 1.0,
            data: DetectionData::KeyValue(HashMap::new()),
            suggested_strategies: vec![],
        });

        assert!(report.has("rust"));
        assert!(!report.has("python"));
    }
}

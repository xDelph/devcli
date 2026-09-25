//! Detection engine for orchestrating strategy execution

use crate::{
    context::{DetectionContext, DetectionScope},
    graph::DependencyGraph,
    registry::StrategyRegistry,
    types::*,
    Result,
};
use std::collections::HashSet;
use std::path::Path;

/// Configuration for detection
#[derive(Debug, Clone)]
pub struct DetectionConfig {
    /// Maximum depth to search directories
    pub max_depth: usize,

    /// Patterns to ignore (e.g., node_modules, .git)
    pub ignore_patterns: Vec<String>,

    /// Categories to enable (None = all)
    pub enabled_categories: Option<HashSet<StrategyCategory>>,

    /// Specific strategies to disable
    pub disabled_strategies: HashSet<String>,

    /// Run strategies in parallel
    pub parallel: bool,

    /// Enable hierarchical workspace detection for monorepos (default: true)
    pub enable_workspace_detection: bool,

    /// Maximum recursion depth for workspace detection (default: 2)
    pub max_workspace_depth: usize,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            ignore_patterns: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
            ],
            enabled_categories: None,
            disabled_strategies: HashSet::new(),
            parallel: false,
            enable_workspace_detection: true,
            max_workspace_depth: 2,
        }
    }
}

/// Main detection engine that orchestrates strategy execution
pub struct DetectionEngine {
    registry: StrategyRegistry,
    config: DetectionConfig,
}

impl DetectionEngine {
    /// Create a new detection engine with a strategy registry
    pub fn new(registry: StrategyRegistry) -> Self {
        Self {
            registry,
            config: DetectionConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(registry: StrategyRegistry, config: DetectionConfig) -> Self {
        Self { registry, config }
    }

    /// Detect with default configuration
    pub fn detect(&self, path: impl AsRef<Path>) -> Result<DetectionReport> {
        self.detect_with_config(path, &self.config)
    }

    /// Detect with custom configuration
    ///
    /// Performs three-phase detection:
    /// - Phase 1: App Type detection (what the application IS)
    /// - Phase 1.5: Workspace Detection (recurse into monorepo workspaces)
    /// - Phase 2: Environment Capability detection (how the application CAN RUN)
    pub fn detect_with_config(
        &self,
        path: impl AsRef<Path>,
        config: &DetectionConfig,
    ) -> Result<DetectionReport> {
        let scope = DetectionScope::root(config.max_workspace_depth);
        self.detect_scoped(path.as_ref(), config, scope)
    }

    /// Internal detection method with explicit scope for hierarchical recursion
    fn detect_scoped(
        &self,
        path: &Path,
        config: &DetectionConfig,
        scope: DetectionScope,
    ) -> Result<DetectionReport> {
        let context = DetectionContext::with_scope(path, scope.clone())?;
        let mut report = DetectionReport::new(context.root_path.clone());

        tracing::debug!(
            path = %path.display(),
            depth = scope.depth,
            "Starting detection"
        );

        // Phase 1: Find and execute App Type strategies
        tracing::debug!(depth = scope.depth, "Phase 1: Detecting app types");

        let app_type_applicable = self.find_app_type_strategies(&context, config);
        tracing::debug!(
            count = app_type_applicable.len(),
            "Found applicable app type strategies"
        );

        let app_type_order = self.resolve_execution_order(&app_type_applicable)?;

        for strategy_id in app_type_order {
            self.execute_strategy(&strategy_id, &context, &mut report);
        }

        tracing::debug!(
            detected = report.app_types().len(),
            "Phase 1 complete: {} app type(s) detected",
            report.app_types().len()
        );

        // Phase 1.5: Workspace Detection (only at root and if enabled)
        if config.enable_workspace_detection && scope.can_recurse(path) {
            let workspaces = self.extract_workspaces(&report);
            if !workspaces.is_empty() {
                tracing::debug!(
                    workspace_count = workspaces.len(),
                    "Phase 1.5: Detecting {} workspace(s)",
                    workspaces.len()
                );
                let children = self.detect_workspaces(workspaces, path, config, &scope)?;
                for child in children {
                    report.add_child(child);
                }
                tracing::debug!(
                    children = report.children.len(),
                    "Phase 1.5 complete: {} workspace(s) detected",
                    report.children.len()
                );
            }
        }

        // Phase 2: Find and execute Environment Capability strategies
        // Re-evaluate can_apply now that app types are detected
        tracing::debug!(
            depth = scope.depth,
            "Phase 2: Detecting environment capabilities"
        );

        let env_applicable = self.find_env_capability_strategies(&context, config);
        tracing::debug!(
            count = env_applicable.len(),
            "Found applicable environment strategies"
        );

        let env_order = self.resolve_execution_order(&env_applicable)?;

        for strategy_id in env_order {
            self.execute_strategy(&strategy_id, &context, &mut report);
        }

        tracing::debug!(
            detected = report.env_capabilities().len(),
            "Phase 2 complete: {} environment(s) detected",
            report.env_capabilities().len()
        );

        tracing::debug!(total_results = report.results.len(), "Detection complete");

        Ok(report)
    }

    /// Extract workspace paths from monorepo detection results
    fn extract_workspaces(&self, report: &DetectionReport) -> Vec<WorkspaceInfo> {
        for result in &report.results {
            if let DetectionData::Monorepo(monorepo_info) = &result.data {
                if !monorepo_info.workspace_info.is_empty() {
                    return monorepo_info.workspace_info.clone();
                }
                // Fallback: convert legacy workspaces strings to WorkspaceInfo
                if !monorepo_info.workspaces.is_empty() {
                    return monorepo_info
                        .workspaces
                        .iter()
                        .map(|path| WorkspaceInfo {
                            path: path.clone(),
                            name: path.split('/').next_back().map(|s| s.to_string()),
                            should_detect: true,
                        })
                        .collect();
                }
            }
        }
        vec![]
    }

    /// Run detection on each workspace in a monorepo, returning child reports
    fn detect_workspaces(
        &self,
        workspaces: Vec<WorkspaceInfo>,
        monorepo_root: &Path,
        config: &DetectionConfig,
        parent_scope: &DetectionScope,
    ) -> Result<Vec<DetectionReport>> {
        let mut children = Vec::new();

        for workspace in workspaces {
            if !workspace.should_detect {
                tracing::debug!(workspace = %workspace.path, "Skipping workspace (should_detect=false)");
                continue;
            }

            let workspace_path = monorepo_root.join(&workspace.path);

            if !workspace_path.exists() || !workspace_path.is_dir() {
                tracing::warn!(workspace = %workspace.path, "Workspace path does not exist or is not a directory");
                continue;
            }

            if !parent_scope.can_recurse(&workspace_path) {
                tracing::debug!(
                    workspace = %workspace.path,
                    depth = parent_scope.depth,
                    "Skipping workspace: recursion limit reached or cycle detected"
                );
                continue;
            }

            let workspace_name = workspace
                .name
                .clone()
                .unwrap_or_else(|| workspace.path.clone());

            let child_scope = DetectionScope::workspace(
                parent_scope,
                workspace_name.clone(),
                monorepo_root.to_path_buf(),
            );

            tracing::debug!(
                workspace = %workspace_name,
                path = %workspace_path.display(),
                "Detecting workspace"
            );

            match self.detect_scoped(&workspace_path, config, child_scope) {
                Ok(child_report) => children.push(child_report),
                Err(e) => {
                    tracing::warn!(
                        workspace = %workspace_name,
                        error = %e,
                        "Workspace detection failed"
                    );
                }
            }
        }

        Ok(children)
    }

    /// Execute a single strategy and handle results
    fn execute_strategy(
        &self,
        strategy_id: &str,
        context: &DetectionContext,
        report: &mut DetectionReport,
    ) {
        if let Some(strategy) = self.registry.get(strategy_id) {
            tracing::debug!(
                strategy = %strategy_id,
                category = ?strategy.category(),
                "Executing strategy"
            );

            match strategy.detect(context) {
                Ok(result) => {
                    tracing::debug!(
                        strategy = %strategy_id,
                        confidence = result.confidence,
                        "Strategy succeeded"
                    );
                    context.store_result(result.clone());
                    report.add_result(result);
                }
                Err(e) => {
                    tracing::warn!(
                        strategy = %strategy_id,
                        error = %e,
                        "Strategy detection failed"
                    );
                }
            }
        }
    }

    /// Find all app type strategies that can apply
    fn find_app_type_strategies(
        &self,
        context: &DetectionContext,
        config: &DetectionConfig,
    ) -> Vec<String> {
        self.registry
            .all()
            .iter()
            .filter(|s| {
                // Only app type strategies
                if !s.category().is_app_type() {
                    return false;
                }

                // Check if category is enabled
                if let Some(ref enabled) = config.enabled_categories {
                    if !enabled.contains(&s.category()) {
                        return false;
                    }
                }

                // Check if strategy is explicitly disabled
                if config.disabled_strategies.contains(s.id()) {
                    return false;
                }

                // Run quick check
                s.can_apply(context)
            })
            .map(|s| s.id().to_string())
            .collect()
    }

    /// Find all environment capability strategies that can apply
    fn find_env_capability_strategies(
        &self,
        context: &DetectionContext,
        config: &DetectionConfig,
    ) -> Vec<String> {
        self.registry
            .all()
            .iter()
            .filter(|s| {
                // Only environment capability strategies
                if !s.category().is_env_capability() {
                    return false;
                }

                // Check if category is enabled
                if let Some(ref enabled) = config.enabled_categories {
                    if !enabled.contains(&s.category()) {
                        return false;
                    }
                }

                // Check if strategy is explicitly disabled
                if config.disabled_strategies.contains(s.id()) {
                    return false;
                }

                // Run quick check (now app types are available in context)
                s.can_apply(context)
            })
            .map(|s| s.id().to_string())
            .collect()
    }

    /// Resolve execution order based on dependencies and priorities
    fn resolve_execution_order(&self, applicable: &[String]) -> Result<Vec<String>> {
        // Filter out conflicting strategies (higher priority wins)
        let mut filtered_applicable = applicable.to_vec();
        let mut to_remove = std::collections::HashSet::new();

        for id in applicable {
            if let Some(strategy) = self.registry.get(id) {
                for conflict in strategy.conflicts_with() {
                    if applicable.contains(&conflict.to_string()) {
                        // Both strategies are applicable - higher priority wins
                        if let Some(conflict_strategy) = self.registry.get(conflict) {
                            if strategy.priority() < conflict_strategy.priority() {
                                // Current strategy has higher priority (lower number) - remove conflict
                                tracing::debug!(
                                    kept = %id,
                                    removed = %conflict,
                                    "Conflict resolved: {} (priority {}) wins over {} (priority {})",
                                    id, strategy.priority(), conflict, conflict_strategy.priority()
                                );
                                to_remove.insert(conflict.to_string());
                            } else {
                                // Conflict strategy has higher priority - remove current
                                tracing::debug!(
                                    kept = %conflict,
                                    removed = %id,
                                    "Conflict resolved: {} (priority {}) wins over {} (priority {})",
                                    conflict, conflict_strategy.priority(), id, strategy.priority()
                                );
                                to_remove.insert(id.clone());
                            }
                        }
                    }
                }
            }
        }

        // Remove conflicting strategies
        filtered_applicable.retain(|id| !to_remove.contains(id));

        let mut graph = DependencyGraph::new();

        // Build dependency graph with filtered strategies
        for id in &filtered_applicable {
            if let Some(strategy) = self.registry.get(id) {
                // Add node with priority
                graph.add_node(id.clone(), strategy.priority());

                // Add dependency edges
                for dep in strategy.depends_on() {
                    // Only add edge if dependency is in filtered list
                    if filtered_applicable.contains(&dep.to_string()) {
                        graph.add_edge(id.clone(), dep.to_string());
                    } else {
                        // Check if this is a cross-phase dependency (e.g., local-env depends on nodejs)
                        // Cross-phase deps are resolved via ctx.get_result() at runtime, not via ordering.
                        let is_cross_phase = self
                            .registry
                            .get(dep)
                            .map(|s| {
                                s.category().is_app_type() != strategy.category().is_app_type()
                            })
                            .unwrap_or(false);

                        if is_cross_phase {
                            tracing::debug!(
                                strategy = %id,
                                missing_dependency = %dep,
                                "Cross-phase dependency (resolved at runtime via context)"
                            );
                        } else {
                            tracing::warn!(
                                strategy = %id,
                                missing_dependency = %dep,
                                "Required dependency not detected or disabled"
                            );
                        }
                    }
                }
            }
        }

        // Perform topological sort with priority ordering
        graph.topological_sort()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::DetectionStrategy;
    use std::collections::HashMap;

    // Test strategy with configurable dependencies
    struct TestStrategy {
        id: String,
        priority: usize,
        deps: Vec<String>,
    }

    impl DetectionStrategy for TestStrategy {
        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.id
        }

        fn category(&self) -> StrategyCategory {
            StrategyCategory::AppType(crate::types::AppTypeCategory::Language)
        }

        fn priority(&self) -> usize {
            self.priority
        }

        fn can_apply(&self, _ctx: &DetectionContext) -> bool {
            true
        }

        fn detect(&self, _ctx: &DetectionContext) -> Result<DetectionResult> {
            Ok(DetectionResult {
                strategy_id: self.id.clone(),
                category: self.category(),
                confidence: 1.0,
                data: DetectionData::KeyValue(HashMap::new()),
                suggested_strategies: vec![],
            })
        }

        fn depends_on(&self) -> Vec<&str> {
            self.deps.iter().map(|s| s.as_str()).collect()
        }
    }

    #[test]
    fn test_execution_order_respects_dependencies() {
        let mut registry = StrategyRegistry::new();

        // B depends on A
        registry.register(Box::new(TestStrategy {
            id: "A".to_string(),
            priority: 200,
            deps: vec![],
        }));

        registry.register(Box::new(TestStrategy {
            id: "B".to_string(),
            priority: 100, // Higher priority than A
            deps: vec!["A".to_string()],
        }));

        let engine = DetectionEngine::new(registry);
        let order = engine
            .resolve_execution_order(&["A".to_string(), "B".to_string()])
            .unwrap();

        // A should execute before B despite B having higher priority
        assert_eq!(order, vec!["A", "B"]);
    }

    #[test]
    fn test_priority_ordering_without_dependencies() {
        let mut registry = StrategyRegistry::new();

        registry.register(Box::new(TestStrategy {
            id: "low".to_string(),
            priority: 300,
            deps: vec![],
        }));

        registry.register(Box::new(TestStrategy {
            id: "high".to_string(),
            priority: 100,
            deps: vec![],
        }));

        registry.register(Box::new(TestStrategy {
            id: "medium".to_string(),
            priority: 200,
            deps: vec![],
        }));

        let engine = DetectionEngine::new(registry);
        let order = engine
            .resolve_execution_order(&["low".to_string(), "high".to_string(), "medium".to_string()])
            .unwrap();

        // Should be ordered by priority
        assert_eq!(order, vec!["high", "medium", "low"]);
    }
}

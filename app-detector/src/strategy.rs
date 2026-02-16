//! Core trait for detection strategies

use crate::{context::DetectionContext, types::*, Result};

/// Core trait for all detection strategies
///
/// Each strategy is responsible for detecting ONE specific concern
/// (a language, a framework, a build tool, etc.)
///
/// # Example
///
/// ```
/// use app_detector::{DetectionStrategy, DetectionContext, DetectionResult, StrategyCategory};
/// use app_detector::types::AppTypeCategory;
///
/// pub struct RustStrategy;
///
/// impl DetectionStrategy for RustStrategy {
///     fn id(&self) -> &str { "rust" }
///     fn name(&self) -> &str { "Rust" }
///     fn category(&self) -> StrategyCategory {
///         StrategyCategory::AppType(AppTypeCategory::Language)
///     }
///     fn priority(&self) -> usize { 100 }
///
///     fn can_apply(&self, ctx: &DetectionContext) -> bool {
///         ctx.file_exists("Cargo.toml")
///     }
///
///     fn detect(&self, ctx: &DetectionContext) -> app_detector::Result<DetectionResult> {
///         // Detection logic here
///         todo!()
///     }
/// }
/// ```
pub trait DetectionStrategy: Send + Sync {
    /// Unique identifier for this strategy
    ///
    /// Should be lowercase, alphanumeric with hyphens (e.g., "nodejs", "react", "docker")
    fn id(&self) -> &str;

    /// Human-readable name for this strategy
    ///
    /// Used for display purposes (e.g., "Node.js", "React", "Docker")
    fn name(&self) -> &str;

    /// Strategy category for organization and filtering
    fn category(&self) -> StrategyCategory;

    /// Priority for execution order (lower number = higher priority)
    ///
    /// Typical values:
    /// - 0-99: Core infrastructure (package managers, runtimes)
    /// - 100-199: Languages
    /// - 200-299: Frameworks
    /// - 300-399: Build tools
    /// - 400-499: Containers/orchestration
    /// - 500+: Everything else
    fn priority(&self) -> usize;

    /// Quick check: Can this strategy potentially apply?
    ///
    /// Should be FAST (< 1ms) - just check file existence, don't read content.
    /// Only called once during the filtering phase.
    ///
    /// # Example
    ///
    /// ```
    /// use app_detector::{DetectionContext, DetectionStrategy};
    /// use tempfile::TempDir;
    /// use std::fs;
    ///
    /// struct MyStrategy;
    ///
    /// impl DetectionStrategy for MyStrategy {
    ///     fn id(&self) -> &str { "my" }
    ///     fn name(&self) -> &str { "My" }
    ///     fn category(&self) -> app_detector::types::StrategyCategory {
    ///         app_detector::types::StrategyCategory::AppType(
    ///             app_detector::types::AppTypeCategory::Language
    ///         )
    ///     }
    ///     fn priority(&self) -> usize { 100 }
    ///     fn can_apply(&self, ctx: &DetectionContext) -> bool {
    ///         ctx.file_exists("Cargo.toml")
    ///     }
    ///     fn detect(&self, _ctx: &DetectionContext) -> app_detector::Result<app_detector::types::DetectionResult> {
    ///         todo!()
    ///     }
    /// }
    ///
    /// // Test it
    /// let temp = TempDir::new().unwrap();
    /// fs::write(temp.path().join("Cargo.toml"), "").unwrap();
    /// let ctx = DetectionContext::new(temp.path()).unwrap();
    /// let strategy = MyStrategy;
    /// assert!(strategy.can_apply(&ctx));
    /// ```
    fn can_apply(&self, ctx: &DetectionContext) -> bool;

    /// Full detection: Extract detailed information
    ///
    /// Only called if `can_apply()` returns true.
    /// Can read files, parse configurations, etc.
    ///
    /// # Errors
    ///
    /// Return an error if detection fails (missing expected files, parse errors, etc.)
    /// The engine will log the error and continue with other strategies.
    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult>;

    /// Optional: Dependencies on other strategies
    ///
    /// Return the IDs of strategies that must run before this one.
    ///
    /// # Example
    ///
    /// ```
    /// # use app_detector::DetectionStrategy;
    /// # struct ReactStrategy;
    /// # impl DetectionStrategy for ReactStrategy {
    /// #     fn id(&self) -> &str { "react" }
    /// #     fn name(&self) -> &str { "React" }
    /// #     fn category(&self) -> app_detector::types::StrategyCategory {
    /// #         app_detector::types::StrategyCategory::AppType(
    /// #             app_detector::types::AppTypeCategory::Framework
    /// #         )
    /// #     }
    /// #     fn priority(&self) -> usize { 200 }
    /// #     fn can_apply(&self, _ctx: &app_detector::DetectionContext) -> bool { true }
    /// #     fn detect(&self, _ctx: &app_detector::DetectionContext) -> app_detector::Result<app_detector::types::DetectionResult> {
    /// #         todo!()
    /// #     }
    /// // React requires Node.js to be detected first
    /// fn depends_on(&self) -> Vec<&str> {
    ///     vec!["nodejs"]
    /// }
    /// # }
    /// ```
    fn depends_on(&self) -> Vec<&str> {
        vec![]
    }

    /// Optional: Strategies that conflict with this one
    ///
    /// Return the IDs of strategies that cannot coexist with this one.
    ///
    /// # Example
    ///
    /// ```
    /// # use app_detector::DetectionStrategy;
    /// # struct NpmStrategy;
    /// # impl DetectionStrategy for NpmStrategy {
    /// #     fn id(&self) -> &str { "npm" }
    /// #     fn name(&self) -> &str { "npm" }
    /// #     fn category(&self) -> app_detector::types::StrategyCategory {
    /// #         app_detector::types::StrategyCategory::AppType(
    /// #             app_detector::types::AppTypeCategory::PackageManager
    /// #         )
    /// #     }
    /// #     fn priority(&self) -> usize { 300 }
    /// #     fn can_apply(&self, _ctx: &app_detector::DetectionContext) -> bool { true }
    /// #     fn detect(&self, _ctx: &app_detector::DetectionContext) -> app_detector::Result<app_detector::types::DetectionResult> {
    /// #         todo!()
    /// #     }
    /// // npm conflicts with yarn
    /// fn conflicts_with(&self) -> Vec<&str> {
    ///     vec!["yarn", "pnpm"]
    /// }
    /// # }
    /// ```
    fn conflicts_with(&self) -> Vec<&str> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test strategy for testing
    struct TestStrategy;

    impl DetectionStrategy for TestStrategy {
        fn id(&self) -> &str {
            "test"
        }

        fn name(&self) -> &str {
            "Test Strategy"
        }

        fn category(&self) -> StrategyCategory {
            StrategyCategory::AppType(crate::types::AppTypeCategory::Language)
        }

        fn priority(&self) -> usize {
            100
        }

        fn can_apply(&self, _ctx: &DetectionContext) -> bool {
            true
        }

        fn detect(&self, _ctx: &DetectionContext) -> Result<DetectionResult> {
            Ok(DetectionResult {
                strategy_id: self.id().to_string(),
                category: self.category(),
                confidence: 1.0,
                data: DetectionData::KeyValue(std::collections::HashMap::new()),
                suggested_strategies: vec![],
            })
        }
    }

    #[test]
    fn test_strategy_trait() {
        let strategy = TestStrategy;
        assert_eq!(strategy.id(), "test");
        assert_eq!(strategy.name(), "Test Strategy");
        assert_eq!(strategy.priority(), 100);
        assert_eq!(strategy.depends_on().len(), 0);
        assert_eq!(strategy.conflicts_with().len(), 0);
    }
}

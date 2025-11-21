// Unit tests for validation and error handling
// Tests circular dependency detection, missing dependencies, and error cases

#[cfg(test)]
mod tests {
    use crate::config::*;
    use crate::test_utils::{AppBuilder, ConfigBuilder};

    // Test: Circular dependency handling (A->B->A)
    // Note: Current implementation uses BFS with visited set, which prevents infinite loops
    // but doesn't explicitly detect/report circular dependencies
    #[test]
    fn test_circular_dependency_simple() {
        // A depends on B
        let app_a = AppBuilder::new("nodejs", "/tmp/a")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .with_dependency("test", "b")
            .build();
        
        // B depends on A (circular!)
        let app_b = AppBuilder::new("nodejs", "/tmp/b")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .with_dependency("test", "a")
            .build();
        
        let config = ConfigBuilder::new()
            .with_app("test", "a", app_a)
            .with_app("test", "b", app_b)
            .build();
        
        // Current implementation handles this gracefully (no infinite loop)
        // It returns both A and B in the dependency chain
        let a_app = resolve_app(&config, "a", None).unwrap();
        let result = dependencies::resolve_dependency_chain(&config, &a_app);
        
        // Should succeed (visited set prevents infinite loop)
        assert!(result.is_ok());
        let deps = result.unwrap();
        
        // Should have B in the chain
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].app_name, "b");
    }

    // Test: Complex circular dependency handling (A->B->C->A)
    // Current implementation handles this without explicit cycle detection
    #[test]
    fn test_circular_dependency_complex() {
        // A depends on B
        let app_a = AppBuilder::new("nodejs", "/tmp/a")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .with_dependency("test", "b")
            .build();
        
        // B depends on C
        let app_b = AppBuilder::new("nodejs", "/tmp/b")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .with_dependency("test", "c")
            .build();
        
        // C depends on A (circular!)
        let app_c = AppBuilder::new("nodejs", "/tmp/c")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .with_dependency("test", "a")
            .build();
        
        let config = ConfigBuilder::new()
            .with_app("test", "a", app_a)
            .with_app("test", "b", app_b)
            .with_app("test", "c", app_c)
            .build();
        
        // Should handle gracefully (visited set prevents infinite loop)
        let a_app = resolve_app(&config, "a", None).unwrap();
        let result = dependencies::resolve_dependency_chain(&config, &a_app);
        assert!(result.is_ok());
        
        let deps = result.unwrap();
        // Should have B and C in the chain
        assert_eq!(deps.len(), 2);
    }

    // Test: Missing dependency app
    #[test]
    fn test_missing_dependency() {
        // App depends on non-existent dependency
        let app = AppBuilder::new("nodejs", "/tmp/app")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .with_dependency("test", "nonexistent")
            .build();
        
        let config = ConfigBuilder::new()
            .with_app("test", "app", app)
            .build();
        
        // Should fail when resolving dependency chain
        let app = resolve_app(&config, "app", None).unwrap();
        let result = dependencies::resolve_dependency_chain(&config, &app);
        assert!(result.is_err());
    }
}

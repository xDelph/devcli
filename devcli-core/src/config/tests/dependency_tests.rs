// Unit tests for dependency chain resolution
// Tests dependencies::resolve_dependency_chain functionality

#[cfg(test)]
mod tests {
    use crate::config::*;
    use crate::test_utils::{AppBuilder, ConfigBuilder};

    // Helper: Create config with multiple projects and apps
    fn create_multi_project_config() -> Config {
        // Project 1: infrastructure
        let redis = AppBuilder::new("redis", "/tmp/redis")
            .with_local_command("start", "redis-server")
            .with_local_default("start")
            .build();

        // Project 2: api (depends on redis)
        let api = AppBuilder::new("nodejs", "/tmp/api")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .with_dependency("infrastructure", "redis")
            .build();

        ConfigBuilder::new()
            .with_app("infrastructure", "redis", redis)
            .with_app("api-project", "api", api)
            .build()
    }

    // Test: Simple dependency chain
    #[test]
    fn test_simple_dependency_chain() {
        let config = create_multi_project_config();
        let api_app = resolve_app(&config, "api", None).unwrap();
        let deps = dependencies::resolve_dependency_chain(&config, &api_app).unwrap();

        // Should have 1 dependency (redis)
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].app_name, "redis");
    }

    // Test: Complex dependency chain (A->B->C)
    #[test]
    fn test_complex_dependency_chain() {
        // C: no dependencies
        let c = AppBuilder::new("test", "/tmp/c")
            .with_local_command("start", "echo c")
            .with_local_default("start")
            .build();

        // B: depends on C
        let b = AppBuilder::new("test", "/tmp/b")
            .with_local_command("start", "echo b")
            .with_local_default("start")
            .with_dependency("test", "c")
            .build();

        // A: depends on B
        let a = AppBuilder::new("test", "/tmp/a")
            .with_local_command("start", "echo a")
            .with_local_default("start")
            .with_dependency("test", "b")
            .build();

        let config = ConfigBuilder::new()
            .with_app("test", "c", c)
            .with_app("test", "b", b)
            .with_app("test", "a", a)
            .build();

        // Resolve dependency chain for A
        let a_app = resolve_app(&config, "a", None).unwrap();
        let deps = dependencies::resolve_dependency_chain(&config, &a_app).unwrap();

        // Should have 2 dependencies (B and C)
        assert_eq!(deps.len(), 2);

        // Verify both B and C are in the chain
        let dep_names: Vec<&str> = deps.iter().map(|d| d.app_name.as_str()).collect();
        assert!(dep_names.contains(&"c"));
        assert!(dep_names.contains(&"b"));
    }

    // Test: Diamond dependency (A depends on B and C, both depend on D)
    #[test]
    fn test_diamond_dependency() {
        // D: no dependencies (base)
        let d = AppBuilder::new("test", "/tmp/d")
            .with_local_command("start", "echo d")
            .with_local_default("start")
            .build();

        // B depends on D
        let b = AppBuilder::new("test", "/tmp/b")
            .with_local_command("start", "echo b")
            .with_local_default("start")
            .with_dependency("test", "d")
            .build();

        // C depends on D
        let c = AppBuilder::new("test", "/tmp/c")
            .with_local_command("start", "echo c")
            .with_local_default("start")
            .with_dependency("test", "d")
            .build();

        // A depends on B and C
        let a = AppBuilder::new("test", "/tmp/a")
            .with_local_command("start", "echo a")
            .with_local_default("start")
            .with_dependency("test", "b")
            .with_dependency("test", "c")
            .build();

        let config = ConfigBuilder::new()
            .with_app("test", "d", d)
            .with_app("test", "b", b)
            .with_app("test", "c", c)
            .with_app("test", "a", a)
            .build();

        // Resolve dependency chain for A
        let a_app = resolve_app(&config, "a", None).unwrap();
        let deps = dependencies::resolve_dependency_chain(&config, &a_app).unwrap();

        // Should have D, B, C (D appears only once despite being referenced twice)
        assert_eq!(deps.len(), 3);

        let dep_names: Vec<&str> = deps.iter().map(|d| d.app_name.as_str()).collect();
        assert!(dep_names.contains(&"d"));
        assert!(dep_names.contains(&"b"));
        assert!(dep_names.contains(&"c"));

        // Verify D is only included once (not duplicated)
        assert_eq!(dep_names.iter().filter(|&&n| n == "d").count(), 1);
    }
}

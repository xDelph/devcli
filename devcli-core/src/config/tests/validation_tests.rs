// Unit tests for validation and error handling
// Tests circular dependency detection, missing dependencies, and error cases

#[cfg(test)]
mod tests {
    use crate::config::*;
    use crate::test_utils::{AppBuilder, ConfigBuilder};

    // Test: Circular dependency (A->B->A) is reported as an error
    #[test]
    fn test_circular_dependency_simple() {
        let app_a = AppBuilder::new("nodejs", "/tmp/a")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .with_dependency("test", "b")
            .build();

        let app_b = AppBuilder::new("nodejs", "/tmp/b")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .with_dependency("test", "a")
            .build();

        let config = ConfigBuilder::new()
            .with_app("test", "a", app_a)
            .with_app("test", "b", app_b)
            .build();

        let a_app = resolve_app(&config, "a", None).unwrap();
        let result = dependencies::resolve_dependency_chain(&config, &a_app);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Circular dependency"));
    }

    // Test: Complex circular dependency (A->B->C->A) is reported as an error
    #[test]
    fn test_circular_dependency_complex() {
        let app_a = AppBuilder::new("nodejs", "/tmp/a")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .with_dependency("test", "b")
            .build();

        let app_b = AppBuilder::new("nodejs", "/tmp/b")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .with_dependency("test", "c")
            .build();

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

        let a_app = resolve_app(&config, "a", None).unwrap();
        let result = dependencies::resolve_dependency_chain(&config, &a_app);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Circular dependency"));
    }

    // Test: Missing dependency app
    #[test]
    fn test_missing_dependency() {
        let app = AppBuilder::new("nodejs", "/tmp/app")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .with_dependency("test", "nonexistent")
            .build();

        let config = ConfigBuilder::new().with_app("test", "app", app).build();

        let app = resolve_app(&config, "app", None).unwrap();
        let result = dependencies::resolve_dependency_chain(&config, &app);
        assert!(result.is_err());
    }
}

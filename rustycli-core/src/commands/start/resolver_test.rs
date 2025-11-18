//! Unit tests for the resolver module

#[cfg(test)]
mod tests {
    use super::super::resolver::*;
    use crate::config::models::*;
    use crate::config::resolver::ResolvedApp;
    use std::collections::HashMap;

    fn create_test_app() -> App {
        App {
            app_type: "test".to_string(),
            path: "/tmp".to_string(),
            commands: Commands {
                local: None,
                docker: None,
                k8s: None,
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: None,
                docker: None,
                k8s: None,
            },
        }
    }

    #[test]
    fn test_start_command_args_clone() {
        let args = StartCommandArgs {
            app_names: vec!["test-app".to_string()],
            project: Some("test-project".to_string()),
            env: Some("local".to_string()),
            skip_deps: true,
            silent: false,
        };
        
        let cloned = args.clone();
        assert_eq!(args.app_names, cloned.app_names);
        assert_eq!(args.project, cloned.project);
        assert_eq!(args.env, cloned.env);
        assert_eq!(args.skip_deps, cloned.skip_deps);
    }

    #[test]
    fn test_get_available_environments_all() {
        let app = App {
            app_type: "test".to_string(),
            path: "/tmp".to_string(),
            commands: Commands {
                local: Some(HashMap::new()),
                docker: Some(HashMap::new()),
                k8s: Some(HashMap::new()),
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: None,
                docker: None,
                k8s: None,
            },
        };
        
        let result = super::super::resolver::get_available_environments(&app);
        assert_eq!(result, "local, docker, k8s");
    }

    #[test]
    fn test_get_available_environments_partial() {
        let app = App {
            app_type: "test".to_string(),
            path: "/tmp".to_string(),
            commands: Commands {
                local: Some(HashMap::new()),
                docker: Some(HashMap::new()),
                k8s: None,
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: None,
                docker: None,
                k8s: None,
            },
        };
        
        let result = super::super::resolver::get_available_environments(&app);
        assert_eq!(result, "local, docker");
    }

    #[test]
    fn test_get_available_environments_none() {
        let app = App {
            app_type: "test".to_string(),
            path: "/tmp".to_string(),
            commands: Commands {
                local: None,
                docker: None,
                k8s: None,
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: None,
                docker: None,
                k8s: None,
            },
        };
        
        let result = super::super::resolver::get_available_environments(&app);
        assert_eq!(result, "none");
    }

    #[test]
    fn test_validate_and_get_command_invalid_environment() {
        let app = App {
            app_type: "test".to_string(),
            path: "/tmp".to_string(),
            commands: Commands {
                local: None,
                docker: None,
                k8s: None,
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: None,
                docker: None,
                k8s: None,
            },
        };
        let resolved_app = ResolvedApp {
            app,
            app_name: "test-app".to_string(),
            project: "test-project".to_string(),
        };
        
        let result = super::super::resolver::validate_and_get_command(&resolved_app, "invalid", "test-app");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid environment"));
    }

    #[test]
    fn test_validate_and_get_command_missing_environment() {
        let app = create_test_app(); // No environments configured
        let resolved_app = ResolvedApp {
            app,
            app_name: "test-app".to_string(),
            project: "test-project".to_string(),
        };
        
        let result = super::super::resolver::validate_and_get_command(&resolved_app, "local", "test-app");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not have 'local' environment configured"));
    }

    #[test]
    fn test_validate_and_get_command_missing_default() {
        let mut app = create_test_app();
        app.commands.local = Some(HashMap::new()); // Has environment but no default
        
        let resolved_app = ResolvedApp {
            app,
            app_name: "test-app".to_string(),
            project: "test-project".to_string(),
        };
        
        let result = super::super::resolver::validate_and_get_command(&resolved_app, "local", "test-app");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not have a default command"));
    }

    #[test]
    fn test_validate_and_get_command_missing_command() {
        let mut app = create_test_app();
        app.commands.local = Some(HashMap::new()); // Empty commands
        app.defaults.local = Some("start".to_string()); // Default exists but command doesn't
        
        let resolved_app = ResolvedApp {
            app,
            app_name: "test-app".to_string(),
            project: "test-project".to_string(),
        };
        
        let result = super::super::resolver::validate_and_get_command(&resolved_app, "local", "test-app");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Default command 'start' not found"));
    }

    #[test]
    fn test_validate_and_get_command_success() {
        let mut app = create_test_app();
        let mut commands = HashMap::new();
        commands.insert("start".to_string(), "npm start".to_string());
        app.commands.local = Some(commands);
        app.defaults.local = Some("start".to_string());
        
        let resolved_app = ResolvedApp {
            app,
            app_name: "test-app".to_string(),
            project: "test-project".to_string(),
        };
        
        let result = super::super::resolver::validate_and_get_command(&resolved_app, "local", "test-app");
        assert!(result.is_ok());
        let (command, default_command) = result.unwrap();
        assert_eq!(command, "npm start");
        assert_eq!(default_command, "start");
    }
}
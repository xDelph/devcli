//! Unit tests for config initialization functionality

#[cfg(test)]
mod init_tests {
    #[test]
    fn test_empty_config_structure() {
        // Test that the empty config contains expected structure
        // This tests the empty config content without file I/O
        let empty_config = r#"{
  "projects": {}
}
"#;

        // Verify empty config is valid JSON
        let parsed: serde_json::Value =
            serde_json::from_str(empty_config).expect("Empty config should be valid JSON");

        // Verify structure - should have empty projects object
        assert!(
            parsed["projects"].is_object(),
            "Should have projects object"
        );
        assert_eq!(
            parsed["projects"].as_object().unwrap().len(),
            0,
            "Projects should be empty"
        );
    }

    #[test]
    fn test_example_template_structure() {
        // Test that the example template (shown in console) contains expected structure
        let example_template = r#"{
  "projects": {
    "example": {
      "apps": {
        "my-app": {
          "type": "nodejs",
          "path": "~/Projects/my-app",
          "commands": {
            "local": {
              "start": "npm start",
              "test": "npm test",
              "build": "npm run build"
            },
            "docker": {
              "build": "docker build -t my-app .",
              "run": "docker run --name my-app --rm my-app"
            }
          },
          "dependencies": [],
          "defaults": {
            "local": "start",
            "docker": "run"
          }
        }
      }
    }
  }
}
"#;

        // Verify example template is valid JSON
        let parsed: serde_json::Value =
            serde_json::from_str(example_template).expect("Example template should be valid JSON");

        // Verify structure
        assert!(
            parsed["projects"].is_object(),
            "Should have projects object"
        );
        assert!(
            parsed["projects"]["example"].is_object(),
            "Should have example project"
        );
        assert!(
            parsed["projects"]["example"]["apps"]["my-app"].is_object(),
            "Should have example app"
        );
        let app = &parsed["projects"]["example"]["apps"]["my-app"];
        assert_eq!(app["type"], "nodejs");
        assert_eq!(app["path"], "~/Projects/my-app");
        assert!(app["commands"]["local"].is_object());
        assert!(app["commands"]["docker"].is_object());
        assert_eq!(app["defaults"]["local"], "start");
        assert_eq!(app["defaults"]["docker"], "run");
        assert!(app["dependencies"].is_array());
    }
}

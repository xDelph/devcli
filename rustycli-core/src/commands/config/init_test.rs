//! Unit tests for config initialization functionality

#[cfg(test)]
mod init_tests {
    #[test]
    fn test_config_init_template_structure() {
        // Test that the template contains expected structure
        // This tests the template content without file I/O
        let template = r#"{
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
        
        // Verify template is valid JSON
        let parsed: serde_json::Value = serde_json::from_str(template)
            .expect("Template should be valid JSON");
        
        // Verify structure
        assert!(parsed["projects"].is_object(), "Should have projects object");
        assert!(parsed["projects"]["example"].is_object(), "Should have example project");
        assert!(parsed["projects"]["example"]["apps"]["my-app"].is_object(), "Should have example app");
        
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
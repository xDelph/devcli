/// App Color Manager - Assigns and tracks colors for app names in log viewer
///
/// This module manages color assignment for app names displayed in log views.
/// It ensures consistent colors across multiple panels and provides a good
/// visual distinction between different apps when viewing logs simultaneously.
use ratatui::style::Color;
use std::collections::HashMap;

/// Manages color assignment for app names in log viewer
///
/// Provides consistent color assignment across multiple log panels,
/// ensuring the same app always gets the same color regardless of
/// which panel it appears in or when it's loaded.
#[derive(Debug, Clone)]
pub struct AppColorManager {
    /// Maps app names to their assigned colors
    app_colors: HashMap<String, Color>,
    /// Available colors for assignment (cycling through these)
    available_colors: Vec<Color>,
    /// Index of next color to assign
    next_color_index: usize,
}

impl AppColorManager {
    /// Creates a new color manager with predefined color palette
    ///
    /// The color palette is chosen to provide good contrast and
    /// visual distinction in terminal environments.
    pub fn new() -> Self {
        Self {
            app_colors: HashMap::new(),
            available_colors: vec![
                Color::Cyan,
                Color::Yellow,
                Color::Green,
                Color::Magenta,
                Color::Blue,
                Color::Red,
                Color::LightCyan,
                Color::LightYellow,
                Color::LightGreen,
                Color::LightMagenta,
                Color::LightBlue,
                Color::LightRed,
            ],
            next_color_index: 0,
        }
    }

    /// Gets the color for an app name, assigning one if not already assigned
    ///
    /// # Arguments
    /// * `app_name` - The name of the app to get color for
    ///
    /// # Returns
    /// The color assigned to this app name
    pub fn get_color_for_app(&mut self, app_name: &str) -> Color {
        if let Some(&color) = self.app_colors.get(app_name) {
            // App already has a color assigned
            color
        } else {
            // Assign a new color to this app
            let color = self.available_colors[self.next_color_index];
            self.app_colors.insert(app_name.to_string(), color);

            // Move to next color, cycling back to start if needed
            self.next_color_index = (self.next_color_index + 1) % self.available_colors.len();

            color
        }
    }

    /// Extracts app name from log file path
    ///
    /// Log files follow the pattern: `{project}_{app}_{context}_{date}.log`
    /// This function extracts the app name (second component) from the filename.
    ///
    /// # Arguments
    /// * `log_path` - Path to the log file
    ///
    /// # Returns
    /// The extracted app name, or "unknown" if parsing fails
    ///
    /// # Examples
    /// ```
    /// // For file: "myproject_redis_start_20241210.log"
    /// // Returns: "redis"
    /// ```
    pub fn extract_app_name_from_path(log_path: &std::path::Path) -> String {
        log_path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|filename| {
                // Remove .log extension
                let name_without_ext = filename.strip_suffix(".log").unwrap_or(filename);

                // Split by underscore and get the second part (app name)
                // We need at least 4 parts for the expected format: project_app_context_date
                let parts: Vec<&str> = name_without_ext.split('_').collect();
                if parts.len() >= 4 {
                    Some(parts[1].to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Gets all currently assigned app colors
    ///
    /// Useful for debugging or displaying color assignments to users.
    ///
    /// # Returns
    /// A reference to the internal color mapping
    pub fn get_all_assignments(&self) -> &HashMap<String, Color> {
        &self.app_colors
    }

    /// Clears all color assignments
    ///
    /// Resets the color manager to initial state. Useful when starting
    /// a fresh log viewing session.
    pub fn clear_assignments(&mut self) {
        self.app_colors.clear();
        self.next_color_index = 0;
    }
}

impl Default for AppColorManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_extract_app_name_from_path() {
        let path = PathBuf::from("myproject_redis_start_20241210.log");
        let app_name = AppColorManager::extract_app_name_from_path(&path);
        assert_eq!(app_name, "redis");

        let path = PathBuf::from("frontend_webapp_build_20241210.log");
        let app_name = AppColorManager::extract_app_name_from_path(&path);
        assert_eq!(app_name, "webapp");

        // Test edge cases
        let path = PathBuf::from("invalid_format.log");
        let app_name = AppColorManager::extract_app_name_from_path(&path);
        assert_eq!(app_name, "unknown");

        let path = PathBuf::from("single.log");
        let app_name = AppColorManager::extract_app_name_from_path(&path);
        assert_eq!(app_name, "unknown");
    }

    #[test]
    fn test_color_assignment_consistency() {
        let mut manager = AppColorManager::new();

        // Same app should always get same color
        let color1 = manager.get_color_for_app("redis");
        let color2 = manager.get_color_for_app("redis");
        assert_eq!(color1, color2);

        // Different apps should get different colors (at least initially)
        let color3 = manager.get_color_for_app("webapp");
        assert_ne!(color1, color3);
    }

    #[test]
    fn test_color_cycling() {
        let mut manager = AppColorManager::new();
        let total_colors = manager.available_colors.len();

        // Assign colors to more apps than available colors
        let mut assigned_colors = Vec::new();
        for i in 0..total_colors + 2 {
            let app_name = format!("app{}", i);
            let color = manager.get_color_for_app(&app_name);
            assigned_colors.push(color);
        }

        // Should cycle back to first colors
        assert_eq!(assigned_colors[0], assigned_colors[total_colors]);
        assert_eq!(assigned_colors[1], assigned_colors[total_colors + 1]);
    }
}

//! Color utilities for terminal output
//!
//! Assigns consistent colors to app names for better visual distinction

use colored::{ColoredString, Colorize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Available colors for app names in logs
/// Using bright/bold variants for better visibility
const COLORS: &[fn(&str) -> ColoredString] = &[
    |s| s.bright_blue().bold(),
    |s| s.bright_green().bold(),
    |s| s.bright_yellow().bold(),
    |s| s.bright_magenta().bold(),
    |s| s.bright_cyan().bold(),
    |s| s.bright_red().bold(),
    |s| s.blue().bold(),
    |s| s.green().bold(),
    |s| s.yellow().bold(),
    |s| s.magenta().bold(),
    |s| s.cyan().bold(),
    |s| s.red().bold(),
];

/// Get a consistent color for an app name
///
/// Uses a hash of the app name to deterministically assign a color.
/// The same app name will always get the same color, providing
/// visual consistency across multiple runs.
///
/// # Example
/// ```
/// use devcli_core::utils::colors::colorize_app_name;
///
/// let colored = colorize_app_name("my-api");
/// println!("[{}] Starting...", colored);
/// ```
pub fn colorize_app_name(app_name: &str) -> ColoredString {
    // Hash the app name to get a consistent color
    let mut hasher = DefaultHasher::new();
    app_name.hash(&mut hasher);
    let hash = hasher.finish();

    // Select color based on hash
    let color_index = (hash as usize) % COLORS.len();
    COLORS[color_index](app_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_name_gets_same_color() {
        let color1 = colorize_app_name("test-app").to_string();
        let color2 = colorize_app_name("test-app").to_string();
        assert_eq!(color1, color2);
    }

    #[test]
    fn test_different_names_can_get_different_colors() {
        // This isn't guaranteed due to hash collisions, but very likely
        let color1 = colorize_app_name("app1").to_string();
        let color2 = colorize_app_name("app2").to_string();
        let color3 = colorize_app_name("app3").to_string();

        // At least one should be different
        assert!(color1 != color2 || color2 != color3 || color1 != color3);
    }
}

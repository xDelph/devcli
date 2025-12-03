//! App-related utility functions

/// Helper function to show which environments are configured for an app
///
/// This is used in error messages when a user tries to run a command for an
/// environment that isn't configured (e.g., trying to run in "docker" when only "local" is defined).
/// It returns a comma-separated list of available environments.
pub fn get_available_environments(app: &crate::config::models::App) -> String {
    let envs = app.commands.available_envs();

    if envs.is_empty() {
        "none".to_string()
    } else {
        envs.join(", ")
    }
}

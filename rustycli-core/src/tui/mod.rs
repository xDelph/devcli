// TUI (Terminal User Interface) module
// This module provides an interactive terminal interface for managing applications
// Inspired by GitUI's clean and efficient design

pub mod app;
pub mod state;
pub mod theme;
pub mod views;

// Re-export main types for easier access
pub use app::TuiApp;
pub use state::{AppState, AppStatus, ProjectState, ViewType};
pub use theme::Theme;
pub use views::{MainView, MainTab, PanelFocus};

#[cfg(test)]
mod tests;

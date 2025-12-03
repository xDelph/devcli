// TUI (Terminal User Interface) module
// This module provides an interactive terminal interface for managing applications
// Inspired by GitUI's clean and efficient design

pub mod app;
pub mod debug;
pub mod event_loop;
pub mod input;
pub mod log_manager;
pub mod refresh_manager;
pub mod state;
pub mod terminal;
pub mod theme;
pub mod utils;
pub mod views;
pub mod widgets;

// Re-export main types for easier access
pub use app::TuiApp;
pub use log_manager::{LogFileInfo, LogManager};
pub use state::{AppState, AppStatus, ProjectState, ViewType};
pub use theme::Theme;
pub use views::{LogViewerView, MainTab, MainView, PanelFocus};
pub use widgets::CommandPopup;

#[cfg(test)]
mod tests;

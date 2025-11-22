// Views module for the TUI
// Contains different view implementations for the application
// Each view handles its own rendering and input processing

pub mod log_viewer;
pub mod main_view;

// Re-export main types
pub use log_viewer::LogViewerView;
pub use main_view::{ConfigField, ConfigForm, ConfigMode, MainTab, MainView, PanelFocus};

#[cfg(test)]
mod tests;

// TODO: Fix unclosed delimiters in main_view_tests
// #[cfg(test)]
// mod main_view_tests;

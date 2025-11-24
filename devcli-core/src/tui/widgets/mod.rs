// Widgets module for reusable TUI components
// Contains popup dialogs and other UI elements

pub mod command_popup;
pub mod help_overlay;
pub mod text_editor;

pub use command_popup::CommandPopup;
pub use help_overlay::HelpOverlay;
pub use text_editor::TextEditor;

#[cfg(test)]
mod tests;

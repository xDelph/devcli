// Widgets module for reusable TUI components
// Contains popup dialogs and other UI elements

pub mod command_popup;
pub mod help_overlay;

pub use command_popup::CommandPopup;
pub use help_overlay::HelpOverlay;

#[cfg(test)]
mod tests;

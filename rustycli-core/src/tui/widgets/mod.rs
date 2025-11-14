// Widgets module for reusable TUI components
// Contains popup dialogs and other UI elements

pub mod command_popup;

pub use command_popup::CommandPopup;

#[cfg(test)]
mod tests;

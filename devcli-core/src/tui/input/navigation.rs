use crate::tui::state::AppState;
use crate::tui::views::MainTab;
use anyhow::Result;
use crossterm::event::KeyEvent;
use std::sync::MutexGuard;

/// Trait for handling navigation and normal mode input
pub trait NavigationHandler {
    /// Handles input in normal (non-edit) mode
    fn handle_normal_mode_input(
        &mut self,
        key: KeyEvent,
        state: &mut MutexGuard<AppState>,
    ) -> Result<bool>;

    /// Switches to a specific tab
    fn switch_to_tab(&mut self, tab: MainTab);

    /// Cycles through tabs
    fn cycle_tab(&mut self);

    /// Toggles focus between panels
    fn toggle_panel_focus(&mut self);

    /// Handles Shift+Up navigation
    fn handle_shift_up(&mut self, state: &mut MutexGuard<AppState>);

    /// Handles Shift+Down navigation
    fn handle_shift_down(&mut self, state: &mut MutexGuard<AppState>);

    /// Handles Up navigation
    fn handle_up(&mut self, state: &mut MutexGuard<AppState>);

    /// Handles Down navigation
    fn handle_down(&mut self, state: &mut MutexGuard<AppState>);

    /// Handles Enter key action
    fn handle_enter(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool>;

    // Config tab handlers
    fn handle_config_add(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool>;
    fn handle_config_edit(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool>;
    fn handle_config_edit_app(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool>;
    fn handle_config_edit_deps(&mut self) -> Result<bool>;
    fn handle_config_edit_env_files(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool>;
    fn handle_config_set_default(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool>;
    fn handle_config_delete(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool>;
    fn handle_config_escape(&mut self) -> Result<bool>;

    // Status tab handlers
    fn handle_status_start_stop(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool>;
    fn handle_status_restart(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool>;
}

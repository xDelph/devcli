/// Manages scroll state and navigation for various popups
#[derive(Debug, Default)]
pub struct PopupScrollManager {
    /// Selected dependency index in dependencies popup
    pub selected_dependency_idx: usize,
    /// Selected item index in add dependency popup (flat app list)
    pub selected_add_dep_idx: usize,
    /// Selected env file index (stage) in env files editor
    pub selected_env_file_idx: usize,
}

impl PopupScrollManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all scroll states
    pub fn reset(&mut self) {
        self.selected_dependency_idx = 0;
        self.selected_add_dep_idx = 0;
        self.selected_env_file_idx = 0;
    }

    // Dependencies Popup Navigation

    pub fn scroll_dependencies_up(&mut self) {
        self.selected_dependency_idx = self.selected_dependency_idx.saturating_sub(1);
    }

    pub fn scroll_dependencies_down(&mut self, total_count: usize) {
        if self.selected_dependency_idx < total_count.saturating_sub(1) {
            self.selected_dependency_idx += 1;
        }
    }

    // Add Dependency Popup Navigation - Flat app list

    pub fn scroll_add_dependency_up(&mut self) {
        self.selected_add_dep_idx = self.selected_add_dep_idx.saturating_sub(1);
    }

    pub fn scroll_add_dependency_down(&mut self, total_apps: usize) {
        if self.selected_add_dep_idx < total_apps.saturating_sub(1) {
            self.selected_add_dep_idx += 1;
        }
    }

    // Env Files Popup Navigation

    pub fn scroll_env_files_up(&mut self) {
        self.selected_env_file_idx = self.selected_env_file_idx.saturating_sub(1);
    }

    pub fn scroll_env_files_down(&mut self, total_entries: usize) {
        if self.selected_env_file_idx < total_entries.saturating_sub(1) {
            self.selected_env_file_idx += 1;
        }
    }
}

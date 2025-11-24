// Main view module - split into smaller submodules for better organization

mod config_editor;
mod input_handler;
mod navigation;
mod popups;
mod render;

// Re-export main types
pub use main_view_core::*;

// Core MainView struct and types
mod main_view_core {
    use crate::tui::log_manager::LogManager;
    use crate::tui::state::AppState;
    use crate::tui::theme::Theme;
    use ratatui::Frame;

    /// Main view with tab-based navigation and split-panel layout
    pub struct MainView {
        /// Currently active tab
        pub(crate) active_tab: MainTab,
        /// Which panel has focus (left app list or right details)
        pub(crate) focus: PanelFocus,
        /// Scroll offset for the left panel list
        #[allow(dead_code)]
        pub(crate) list_scroll: usize,
        /// Scroll offset for the right panel details
        pub(crate) detail_scroll: usize,
        /// Selected command index in the Commands tab
        pub(crate) selected_command_idx: usize,
        /// Selected log file index in the Logs tab
        pub(crate) selected_log_idx: usize,
        /// Log manager for discovering log files
        pub(crate) log_manager: LogManager,
        /// Config editor mode (view, add, edit)
        pub(crate) config_mode: ConfigMode,
        /// Form data for adding/editing apps
        pub(crate) config_form: ConfigForm,
        /// Currently focused field in config form
        pub(crate) config_focused_field: ConfigField,
        /// Selected command index in Config tab view mode
        pub(crate) selected_config_command_idx: usize,
        /// Manager for popup scroll states
        pub(crate) popup_scroll_manager:
            crate::tui::views::main_view::popups::scroll_manager::PopupScrollManager,
        /// Delete confirmation message
        pub(crate) delete_confirm_message: String,
        /// Delete confirmation action type
        pub(crate) delete_confirm_type: DeleteType,
    }

    /// Type of deletion being confirmed
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DeleteType {
        App,
        Command,
        Dependency,
        EnvFile,
    }

    /// Config editor modes
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ConfigMode {
        View,
        Add,
        Edit,
        AddCommand,
        EditCommand,
        EditDependencies,
        AddDependency,
        EditEnvFiles,
        AddEnvFile,
        EditEnvFile,
        ConfirmDelete,
    }

    /// Form data for config editor
    #[derive(Debug, Clone, Default)]
    pub struct ConfigForm {
        pub project_name: crate::tui::widgets::TextEditor,
        pub app_name: crate::tui::widgets::TextEditor,
        pub app_type: crate::tui::widgets::TextEditor,
        pub path: crate::tui::widgets::TextEditor,
        pub local_start_cmd: crate::tui::widgets::TextEditor,
        pub docker_start_cmd: crate::tui::widgets::TextEditor,
        pub edit_command_env: String, // Dropdown, keeps as String
        pub edit_command_name: crate::tui::widgets::TextEditor,
        pub edit_command_value: crate::tui::widgets::TextEditor,
        pub env_file_stage: crate::tui::widgets::TextEditor,
        pub env_file_context: crate::tui::widgets::TextEditor,
        pub env_file_path: crate::tui::widgets::TextEditor,
    }

    /// Config form fields
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ConfigField {
        ProjectName,
        AppName,
        AppType,
        Path,
        LocalStartCmd,
        DockerStartCmd,
        EditCommandEnv,
        EditCommandName,
        EditCommandValue,
        EnvFileStage,
        EnvFileContext,
        EnvFilePath,
    }

    /// The four main tabs in the interface
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MainTab {
        Status,
        Commands,
        Logs,
        Config,
    }

    /// Indicates which panel currently has focus
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PanelFocus {
        AppList,
        DetailPanel,
    }

    impl MainView {
        /// Creates a new MainView with default settings
        pub fn new() -> Self {
            Self {
                active_tab: MainTab::Status,
                focus: PanelFocus::AppList,
                list_scroll: 0,
                detail_scroll: 0,
                selected_command_idx: 0,
                selected_log_idx: 0,
                log_manager: LogManager::default(),
                config_mode: ConfigMode::View,
                config_form: ConfigForm::default(),
                config_focused_field: ConfigField::ProjectName,
                selected_config_command_idx: 0,
                popup_scroll_manager:
                    crate::tui::views::main_view::popups::scroll_manager::PopupScrollManager::new(),
                delete_confirm_message: String::new(),
                delete_confirm_type: DeleteType::App,
            }
        }

        /// Renders the main view with all its components
        pub fn render(&self, frame: &mut Frame, state: &AppState, theme: &Theme) {
            // Rendering is handled in the renderer module
            self.render_main(frame, state, theme);
        }

        /// Returns a sorted list of (ProjectName, Vec<AppName>) for dependency selection
        /// Filters out the current app to prevent self-dependency
        pub fn get_sorted_dependency_candidates(
            &self,
            config: &crate::config::models::Config,
            current_project_name: Option<&str>,
            current_app_name: Option<&str>,
        ) -> Vec<(String, Vec<String>)> {
            let mut candidates: Vec<(String, Vec<String>)> = Vec::new();

            // Get all projects and sort them by name
            let mut projects: Vec<_> = config.projects.iter().collect();
            projects.sort_by_key(|(name, _)| *name);

            for (proj_name, project) in projects {
                // Get all apps in the project
                let mut apps: Vec<_> = project
                    .apps
                    .keys()
                    .filter(|app_name| {
                        // Filter out the current app if it's in this project
                        if let (Some(curr_proj), Some(curr_app)) =
                            (current_project_name, current_app_name)
                        {
                            if proj_name == curr_proj && *app_name == curr_app {
                                return false;
                            }
                        }
                        true
                    })
                    .cloned()
                    .collect();

                // Sort apps by name
                apps.sort();

                if !apps.is_empty() {
                    candidates.push((proj_name.clone(), apps));
                }
            }

            candidates
        }

        /// Get app at flat index from dependency candidates
        /// Returns (project_name, app_name) for the given flat index
        pub fn get_app_at_flat_index(
            &self,
            candidates: &[(String, Vec<String>)],
            flat_index: usize,
        ) -> Option<(String, String)> {
            let mut current_index = 0;
            for (proj_name, apps) in candidates {
                if flat_index < current_index + apps.len() {
                    let app_index = flat_index - current_index;
                    return Some((proj_name.clone(), apps[app_index].clone()));
                }
                current_index += apps.len();
            }
            None
        }
    }

    impl Default for MainView {
        fn default() -> Self {
            Self::new()
        }
    }
}

// Main view module - split into smaller submodules for better organization

mod config_editor;
mod input_handler;
mod navigation;
mod renderer;

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
        /// Selected dependency index in dependencies popup
        pub(crate) selected_dependency_idx: usize,
        /// Selected project index when adding dependency
        pub(crate) selected_add_dep_project_idx: usize,
        /// Selected app index when adding dependency
        pub(crate) selected_add_dep_app_idx: usize,
        /// Selected env file index (stage) in env files editor
        pub(crate) selected_env_file_idx: usize,
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
        pub project_name: String,
        pub app_name: String,
        pub app_type: String,
        pub path: String,
        pub local_start_cmd: String,
        pub docker_start_cmd: String,
        pub edit_command_env: String,
        pub edit_command_name: String,
        pub edit_command_value: String,
        pub env_file_stage: String,
        pub env_file_context: String,
        pub env_file_path: String,
        pub cursor_project_name: usize,
        pub cursor_app_name: usize,
        pub cursor_app_type: usize,
        pub cursor_path: usize,
        pub cursor_local_start_cmd: usize,
        pub cursor_docker_start_cmd: usize,
        pub cursor_edit_command_name: usize,
        pub cursor_edit_command_value: usize,
        pub cursor_env_file_stage: usize,
        pub cursor_env_file_context: usize,
        pub cursor_env_file_path: usize,
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
                selected_dependency_idx: 0,
                selected_add_dep_project_idx: 0,
                selected_add_dep_app_idx: 0,
                selected_env_file_idx: 0,
                delete_confirm_message: String::new(),
                delete_confirm_type: DeleteType::App,
            }
        }

        /// Renders the main view with all its components
        pub fn render(&self, frame: &mut Frame, state: &AppState, theme: &Theme) {
            // Rendering is handled in the renderer module
            self.render_main(frame, state, theme);
        }
    }

    impl Default for MainView {
        fn default() -> Self {
            Self::new()
        }
    }
}

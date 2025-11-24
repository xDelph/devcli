// Config editor logic for MainView
// Handles CRUD operations for apps, commands, and dependencies

use super::MainView;
use crate::config::loader::{load_config, save_config};
use crate::config::models::{App, Commands, Defaults, Project};
use crate::process::tracker::ProcessTracker;
use crate::tui::state::{AppState, AppStateData};
use anyhow::Result;
use std::collections::HashMap;

impl MainView {
    /// Loads an app into the config form for editing
    pub(crate) fn load_app_into_form(&mut self, project: &str, app_name: &str, app: &AppStateData) {
        use crate::tui::widgets::TextEditor;
        self.config_form.project_name = TextEditor::with_content(project.to_string());
        self.config_form.app_name = TextEditor::with_content(app_name.to_string());
        self.config_form.app_type = TextEditor::with_content(app.app_type.clone());
        self.config_form.path = TextEditor::with_content(app.path.clone().unwrap_or_default());

        // Load commands if available
        if let Some(local) = app.commands.get("local") {
            if let Some(start_cmd) = local.first() {
                self.config_form.local_start_cmd =
                    TextEditor::with_content(start_cmd.command.clone());
            } else {
                self.config_form.local_start_cmd = TextEditor::default();
            }
        } else {
            self.config_form.local_start_cmd = TextEditor::default();
        }

        if let Some(docker) = app.commands.get("docker") {
            if let Some(start_cmd) = docker.first() {
                self.config_form.docker_start_cmd =
                    TextEditor::with_content(start_cmd.command.clone());
            } else {
                self.config_form.docker_start_cmd = TextEditor::default();
            }
        } else {
            self.config_form.docker_start_cmd = TextEditor::default();
        }
    }

    /// Loads a specific command into the form for editing
    pub(crate) fn load_command_into_form(&mut self, project: &str, app_name: &str) -> Result<()> {
        let config = load_config()?;

        if let Some(proj) = config.projects.get(project) {
            if let Some(app) = proj.apps.get(app_name) {
                let mut global_idx = 0;

                // Check local commands
                if let Some(local) = &app.commands.local {
                    let mut sorted: Vec<_> = local.iter().collect();
                    sorted.sort_by_key(|(name, _)| *name);
                    if self.selected_config_command_idx < global_idx + sorted.len() {
                        let idx = self.selected_config_command_idx - global_idx;
                        let (name, cmd) = sorted[idx];
                        self.load_command_into_form_helper("local", name, cmd);
                        return Ok(());
                    }
                    global_idx += sorted.len();
                }

                // Check docker commands
                if let Some(docker) = &app.commands.docker {
                    let mut sorted: Vec<_> = docker.iter().collect();
                    sorted.sort_by_key(|(name, _)| *name);
                    if self.selected_config_command_idx < global_idx + sorted.len() {
                        let idx = self.selected_config_command_idx - global_idx;
                        let (name, cmd) = sorted[idx];
                        self.load_command_into_form_helper("docker", name, cmd);
                        return Ok(());
                    }
                    global_idx += sorted.len();
                }

                // Check orbstack commands
                if let Some(orbstack) = &app.commands.orbstack {
                    let mut sorted: Vec<_> = orbstack.iter().collect();
                    sorted.sort_by_key(|(name, _)| *name);
                    if self.selected_config_command_idx < global_idx + sorted.len() {
                        let idx = self.selected_config_command_idx - global_idx;
                        let (name, cmd) = sorted[idx];
                        self.load_command_into_form_helper("orbstack", name, cmd);
                        return Ok(());
                    }
                    global_idx += sorted.len();
                }

                // Check k8s commands
                if let Some(k8s) = &app.commands.k8s {
                    let mut sorted: Vec<_> = k8s.iter().collect();
                    sorted.sort_by_key(|(name, _)| *name);
                    if self.selected_config_command_idx < global_idx + sorted.len() {
                        let idx = self.selected_config_command_idx - global_idx;
                        let (name, cmd) = sorted[idx];
                        self.load_command_into_form_helper("k8s", name, cmd);
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }

    fn load_command_into_form_helper(&mut self, env: &str, name: &str, cmd: &str) {
        use super::ConfigField;
        use super::ConfigMode;
        use crate::tui::widgets::TextEditor;

        self.config_form.edit_command_env = env.to_string();
        self.config_form.edit_command_name = TextEditor::with_content(name.to_string());
        self.config_form.edit_command_value = TextEditor::with_content(cmd.to_string());
        self.config_focused_field = ConfigField::EditCommandName;
        self.config_mode = ConfigMode::EditCommand;
    }

    /// Saves the config form
    pub(crate) fn save_config_form(&mut self) -> Result<()> {
        use super::ConfigMode;

        // Validate required fields
        // Validate required fields
        if self.config_form.project_name.content().is_empty()
            || self.config_form.app_name.content().is_empty()
            || self.config_form.app_type.content().is_empty()
            || self.config_form.path.content().is_empty()
        {
            return Ok(()); // Silently ignore invalid forms
        }

        let mut config = load_config()?;
        let app = self.create_app_from_form();

        let project = config
            .projects
            .entry(self.config_form.project_name.content().to_string())
            .or_insert_with(|| Project {
                apps: HashMap::new(),
            });

        project
            .apps
            .insert(self.config_form.app_name.content().to_string(), app);
        save_config(&config)?;
        self.config_mode = ConfigMode::View;

        Ok(())
    }

    /// Reloads the state from the config file
    pub(crate) fn reload_state_from_config(
        &self,
        state: &mut std::sync::MutexGuard<AppState>,
    ) -> Result<()> {
        let config = load_config()?;
        let tracker = ProcessTracker::new()?;
        let new_state = AppState::from_config(&config, &tracker)?;

        let selected_project_idx = state.selected_project_idx;
        let selected_app_idx = state.selected_app_idx;

        **state = new_state;

        if selected_project_idx < state.projects.len() {
            state.selected_project_idx = selected_project_idx;
            if selected_app_idx < state.projects[selected_project_idx].apps.len() {
                state.selected_app_idx = selected_app_idx;
            }
        }

        Ok(())
    }

    /// Saves a new command to an existing app
    pub(crate) fn save_new_command(&mut self, project: &str, app_name: &str) -> Result<()> {
        use super::ConfigMode;

        let mut config = load_config()?;

        if let Some(proj) = config.projects.get_mut(project) {
            if let Some(app) = proj.apps.get_mut(app_name) {
                let command_name = self.config_form.edit_command_name.content().to_string();
                let command_value = self.config_form.edit_command_value.content().to_string();

                match self.config_form.edit_command_env.as_str() {
                    "local" => {
                        let local = app.commands.local.get_or_insert_with(HashMap::new);
                        let is_first = local.is_empty();
                        local.insert(command_name.clone(), command_value);
                        if is_first {
                            app.defaults.local = Some(command_name);
                        }
                    }
                    "docker" => {
                        let docker = app.commands.docker.get_or_insert_with(HashMap::new);
                        let is_first = docker.is_empty();
                        docker.insert(command_name.clone(), command_value);
                        if is_first {
                            app.defaults.docker = Some(command_name);
                        }
                    }
                    "orbstack" => {
                        let orbstack = app.commands.orbstack.get_or_insert_with(HashMap::new);
                        let is_first = orbstack.is_empty();
                        orbstack.insert(command_name.clone(), command_value);
                        if is_first {
                            app.defaults.orbstack = Some(command_name);
                        }
                    }
                    "k8s" => {
                        let k8s = app.commands.k8s.get_or_insert_with(HashMap::new);
                        let is_first = k8s.is_empty();
                        k8s.insert(command_name.clone(), command_value);
                        if is_first {
                            app.defaults.k8s = Some(command_name);
                        }
                    }
                    _ => {}
                }

                save_config(&config)?;
            }
        }

        self.config_mode = ConfigMode::View;
        Ok(())
    }

    /// Saves a command edit
    pub(crate) fn save_command_edit(&mut self, project: &str, app_name: &str) -> Result<()> {
        use super::ConfigMode;

        let mut config = load_config()?;

        if let Some(proj) = config.projects.get_mut(project) {
            if let Some(app) = proj.apps.get_mut(app_name) {
                match self.config_form.edit_command_env.as_str() {
                    "local" => {
                        if let Some(local) = &mut app.commands.local {
                            local.insert(
                                self.config_form.edit_command_name.content().to_string(),
                                self.config_form.edit_command_value.content().to_string(),
                            );
                        }
                    }
                    "docker" => {
                        if let Some(docker) = &mut app.commands.docker {
                            docker.insert(
                                self.config_form.edit_command_name.content().to_string(),
                                self.config_form.edit_command_value.content().to_string(),
                            );
                        }
                    }
                    "orbstack" => {
                        if let Some(orbstack) = &mut app.commands.orbstack {
                            orbstack.insert(
                                self.config_form.edit_command_name.content().to_string(),
                                self.config_form.edit_command_value.content().to_string(),
                            );
                        }
                    }
                    "k8s" => {
                        if let Some(k8s) = &mut app.commands.k8s {
                            k8s.insert(
                                self.config_form.edit_command_name.content().to_string(),
                                self.config_form.edit_command_value.content().to_string(),
                            );
                        }
                    }
                    _ => {}
                }

                save_config(&config)?;
            }
        }

        self.config_mode = ConfigMode::View;
        Ok(())
    }

    /// Creates an App from the form data
    fn create_app_from_form(&self) -> App {
        let mut local_cmds = HashMap::new();
        if !self.config_form.local_start_cmd.content().is_empty() {
            local_cmds.insert(
                "start".to_string(),
                self.config_form.local_start_cmd.content().to_string(),
            );
        }

        let mut docker_cmds = HashMap::new();
        if !self.config_form.docker_start_cmd.content().is_empty() {
            docker_cmds.insert(
                "start".to_string(),
                self.config_form.docker_start_cmd.content().to_string(),
            );
        }

        App {
            app_type: self.config_form.app_type.content().to_string(),
            path: self.config_form.path.content().to_string(),
            commands: Commands {
                local: if local_cmds.is_empty() {
                    None
                } else {
                    Some(local_cmds)
                },
                docker: if docker_cmds.is_empty() {
                    None
                } else {
                    Some(docker_cmds)
                },
                orbstack: None,
                k8s: None,
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: if !self.config_form.local_start_cmd.content().is_empty() {
                    Some("start".to_string())
                } else {
                    None
                },
                docker: if !self.config_form.docker_start_cmd.content().is_empty() {
                    Some("start".to_string())
                } else {
                    None
                },
                orbstack: None,
                k8s: None,
            },
            dockerfile_path: None,
            env_files: None, // No env files configured in TUI editor (will be added in future task)
            default_stages: None, // No default stages configured in TUI editor
        }
    }

    /// Deletes an app from the config
    pub(crate) fn delete_app(&self, project: &str, app: &str) -> Result<()> {
        let mut config = load_config()?;

        if let Some(proj) = config.projects.get_mut(project) {
            proj.apps.remove(app);

            if proj.apps.is_empty() {
                config.projects.remove(project);
            }
        }

        save_config(&config)?;
        Ok(())
    }

    /// Sets the selected command as default for its environment
    pub(crate) fn set_command_as_default(&self, project: &str, app_name: &str) -> Result<()> {
        let mut config = load_config()?;

        if let Some(proj) = config.projects.get_mut(project) {
            if let Some(app) = proj.apps.get_mut(app_name) {
                let mut global_idx = 0;

                // Check local commands
                if let Some(local) = &app.commands.local {
                    let mut sorted: Vec<_> = local.keys().cloned().collect();
                    sorted.sort();
                    if self.selected_config_command_idx < global_idx + sorted.len() {
                        let idx = self.selected_config_command_idx - global_idx;
                        app.defaults.local = Some(sorted[idx].clone());
                        save_config(&config)?;
                        return Ok(());
                    }
                    global_idx += sorted.len();
                }

                // Check docker commands
                if let Some(docker) = &app.commands.docker {
                    let mut sorted: Vec<_> = docker.keys().cloned().collect();
                    sorted.sort();
                    if self.selected_config_command_idx < global_idx + sorted.len() {
                        let idx = self.selected_config_command_idx - global_idx;
                        app.defaults.docker = Some(sorted[idx].clone());
                        save_config(&config)?;
                        return Ok(());
                    }
                    global_idx += sorted.len();
                }

                // Check k8s commands
                if let Some(k8s) = &app.commands.k8s {
                    let mut sorted: Vec<_> = k8s.keys().cloned().collect();
                    sorted.sort();
                    if self.selected_config_command_idx < global_idx + sorted.len() {
                        let idx = self.selected_config_command_idx - global_idx;
                        app.defaults.k8s = Some(sorted[idx].clone());
                        save_config(&config)?;
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }

    /// Removes a dependency from an app
    pub(crate) fn remove_dependency(
        &mut self,
        project: &str,
        app_name: &str,
        dep_idx: usize,
    ) -> Result<()> {
        let mut config = load_config()?;

        let mut new_len = 0;
        if let Some(proj) = config.projects.get_mut(project) {
            if let Some(app) = proj.apps.get_mut(app_name) {
                if dep_idx < app.dependencies.len() {
                    app.dependencies.remove(dep_idx);
                    new_len = app.dependencies.len();
                }
            }
        }

        save_config(&config)?;

        if self.popup_scroll_manager.selected_dependency_idx >= new_len
            && self.popup_scroll_manager.selected_dependency_idx > 0
        {
            self.popup_scroll_manager.selected_dependency_idx -= 1;
        }

        Ok(())
    }

    /// Adds a dependency to an app
    pub(crate) fn add_dependency(
        &mut self,
        project: &str,
        app_name: &str,
        dep_project: &str,
        dep_app: &str,
    ) -> Result<()> {
        use super::ConfigMode;

        let mut config = load_config()?;

        if let Some(proj) = config.projects.get_mut(project) {
            if let Some(app) = proj.apps.get_mut(app_name) {
                let dep = crate::config::models::Dependency {
                    project: dep_project.to_string(),
                    app: dep_app.to_string(),
                };

                if !app
                    .dependencies
                    .iter()
                    .any(|d| d.project == dep.project && d.app == dep.app)
                {
                    app.dependencies.push(dep);
                    save_config(&config)?;
                }
            }
        }

        self.config_mode = ConfigMode::EditDependencies;
        Ok(())
    }

    /// Gets the currently selected command in config view (environment, CommandInfo)
    pub(crate) fn get_selected_config_command<'a>(
        &self,
        app: &'a AppStateData,
    ) -> Option<(&'a str, &'a crate::tui::state::CommandInfo)> {
        use crate::config::models::Environment;
        let mut current_idx = 0;

        // Iterate through environments in standard order
        for env in Environment::all() {
            let env_str = env.as_str();
            if let Some(commands) = app.commands.get(env_str) {
                if self.selected_config_command_idx < current_idx + commands.len() {
                    let cmd_idx = self.selected_config_command_idx - current_idx;
                    return Some((env_str, &commands[cmd_idx]));
                }
                current_idx += commands.len();
            }
        }

        None
    }

    /// Deletes a command from an app
    pub(crate) fn delete_command(
        &mut self,
        project: &str,
        app_name: &str,
        env: &str,
        command_name: &str,
    ) -> Result<()> {
        let mut config = load_config()?;

        if let Some(proj) = config.projects.get_mut(project) {
            if let Some(app) = proj.apps.get_mut(app_name) {
                match env {
                    "local" => {
                        if let Some(local) = &mut app.commands.local {
                            local.remove(command_name);
                            if local.is_empty() {
                                app.commands.local = None;
                            }
                        }
                    }
                    "docker" => {
                        if let Some(docker) = &mut app.commands.docker {
                            docker.remove(command_name);
                            if docker.is_empty() {
                                app.commands.docker = None;
                            }
                        }
                    }
                    "orbstack" => {
                        if let Some(orbstack) = &mut app.commands.orbstack {
                            orbstack.remove(command_name);
                            if orbstack.is_empty() {
                                app.commands.orbstack = None;
                            }
                        }
                    }
                    "k8s" => {
                        if let Some(k8s) = &mut app.commands.k8s {
                            k8s.remove(command_name);
                            if k8s.is_empty() {
                                app.commands.k8s = None;
                            }
                        }
                    }
                    _ => {}
                }

                save_config(&config)?;
            }
        }

        Ok(())
    }
    /// Saves a new or edited environment file configuration
    pub(crate) fn save_env_file(&mut self, app_name: &str) -> Result<()> {
        use super::ConfigMode;
        use crate::commands::env::add_env_file;

        let stage = self.config_form.env_file_stage.content().trim();
        let context = self.config_form.env_file_context.content().trim();
        let file_path = self.config_form.env_file_path.content().trim();

        if stage.is_empty() || context.is_empty() || file_path.is_empty() {
            return Err(anyhow::anyhow!("All fields are required"));
        }

        add_env_file(app_name, stage, context, file_path)?;

        self.config_mode = ConfigMode::EditEnvFiles;
        Ok(())
    }

    /// Loads the selected env file entry into the form for editing
    pub(crate) fn load_selected_env_file(
        &mut self,
        state: &mut std::sync::MutexGuard<AppState>,
    ) -> Result<()> {
        use super::{ConfigField, ConfigMode};

        if let Some(app) = state.selected_app() {
            if let Some(env_files) = &app.env_files {
                // Build flat list of entries
                let mut entries: Vec<(String, String, String)> = Vec::new();
                for (stage, contexts) in env_files {
                    let mut sorted_contexts: Vec<_> = contexts.iter().collect();
                    sorted_contexts.sort_by_key(|(context, _)| context.as_str());

                    for (context, file_path) in sorted_contexts {
                        entries.push((stage.clone(), context.clone(), file_path.clone()));
                    }
                }
                entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

                if let Some((stage, context, file_path)) =
                    entries.get(self.popup_scroll_manager.selected_env_file_idx)
                {
                    use crate::tui::widgets::TextEditor;
                    self.config_form.env_file_stage = TextEditor::with_content(stage.clone());
                    self.config_form.env_file_context = TextEditor::with_content(context.clone());
                    self.config_form.env_file_path = TextEditor::with_content(file_path.clone());
                    self.config_focused_field = ConfigField::EnvFileStage;
                    self.config_mode = ConfigMode::EditEnvFile;
                }
            }
        }
        Ok(())
    }

    /// Prepares deletion of the selected environment file entry
    pub(crate) fn confirm_delete_env_file(
        &mut self,
        state: &mut std::sync::MutexGuard<AppState>,
    ) -> Result<()> {
        use super::{ConfigMode, DeleteType};

        if let Some(app) = state.selected_app() {
            if let Some(env_files) = &app.env_files {
                // Build flat list of entries
                let mut entries: Vec<(String, String, String)> = Vec::new();
                for (stage, contexts) in env_files {
                    let mut sorted_contexts: Vec<_> = contexts.iter().collect();
                    sorted_contexts.sort_by_key(|(context, _)| context.as_str());

                    for (context, file_path) in sorted_contexts {
                        entries.push((stage.clone(), context.clone(), file_path.clone()));
                    }
                }
                entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

                if let Some((stage, context, file_path)) =
                    entries.get(self.popup_scroll_manager.selected_env_file_idx)
                {
                    self.delete_confirm_message = format!(
                        "Delete env file?\n\nStage: {}\nContext: {}\nFile: {}",
                        stage, context, file_path
                    );
                    self.delete_confirm_type = DeleteType::EnvFile;
                    self.config_mode = ConfigMode::ConfirmDelete;
                }
            }
        }
        Ok(())
    }
}

// Config editor logic for MainView
// Handles CRUD operations for apps, commands, and dependencies

use super::MainView;
use crate::config::loader::{load_config, save_config};
use crate::config::models::{App, Config, Environment, Project};
use crate::tui::state::{AppKey, AppState, AppStateData};
use anyhow::{anyhow, bail, Result};

impl MainView {
    /// Loads an app into the config form for editing.
    ///
    /// Everything comes from the config, not from the TUI row: a worktree
    /// row (`proj@slug`) shows its worktree's path, which used to be loaded
    /// here and then saved over the configured path.
    pub(crate) fn load_app_into_form(&mut self, project: &str, app_name: &str) -> Result<()> {
        use crate::tui::widgets::TextEditor;

        let project = configured_project(project).to_string();
        let config = load_config()?;
        let app = config
            .projects
            .get(&project)
            .and_then(|p| p.apps.get(app_name))
            .ok_or_else(|| anyhow!("{project}/{app_name} is not in the config"))?;

        let local = editable_command(app, Environment::Local);
        let docker = editable_command(app, Environment::Docker);
        self.config_form = super::ConfigForm {
            project_name: TextEditor::with_content(project.clone()),
            app_name: TextEditor::with_content(app_name.to_string()),
            app_type: TextEditor::with_content(app.app_type.clone()),
            path: TextEditor::with_content(app.path.clone()),
            local_start_cmd: TextEditor::with_content(
                local
                    .as_ref()
                    .map(|(_, cmd)| cmd.clone())
                    .unwrap_or_default(),
            ),
            docker_start_cmd: TextEditor::with_content(
                docker
                    .as_ref()
                    .map(|(_, cmd)| cmd.clone())
                    .unwrap_or_default(),
            ),
            local_cmd_name: local.map(|(name, _)| name),
            docker_cmd_name: docker.map(|(name, _)| name),
            editing: Some(AppKey::new(project, app_name)),
            ..Default::default()
        };
        Ok(())
    }

    /// Loads a specific command into the form for editing
    pub(crate) fn load_command_into_form(&mut self, project: &str, app_name: &str) -> Result<()> {
        let project = configured_project(project);
        let config = load_config()?;

        if let Some(proj) = config.projects.get(project)
            && let Some(app) = proj.apps.get(app_name)
        {
            let mut global_idx = 0;

            // Same order as the config panel renders them.
            for env in crate::config::models::Environment::all() {
                let Some(commands) = app.commands.get(env.as_str()) else {
                    continue;
                };
                let mut sorted: Vec<_> = commands.iter().collect();
                sorted.sort_by_key(|(name, _)| *name);
                if self.selected_config_command_idx < global_idx + sorted.len() {
                    let idx = self.selected_config_command_idx - global_idx;
                    let (name, cmd) = sorted[idx];
                    self.load_command_into_form_helper(env.as_str(), name, cmd);
                    return Ok(());
                }
                global_idx += sorted.len();
            }
        }

        Ok(())
    }

    fn load_command_into_form_helper(&mut self, env: &str, name: &str, cmd: &str) {
        use super::ConfigField;
        use super::ConfigMode;
        use crate::tui::widgets::TextEditor;

        self.config_form.edit_command_env = env.to_string();
        self.config_form.edit_command_original = Some(name.to_string());
        self.config_form.edit_command_name = TextEditor::with_content(name.to_string());
        self.config_form.edit_command_value = TextEditor::with_content(cmd.to_string());
        self.config_focused_field = ConfigField::EditCommandName;
        self.config_mode = ConfigMode::EditCommand;
    }

    /// Saves the "Edit app" form and returns the app's (new) identity.
    ///
    /// The app is updated in place: only the fields of the form change. It
    /// used to be replaced by an app built from the six fields, which dropped
    /// its dependencies, env files, health check, restart policy, port,
    /// stages, every other command (the first local one, alphabetically,
    /// came back renamed `start`) and unknown keys. A new project or app name
    /// moves the entry (the old one used to stay, duplicated).
    pub(crate) fn save_config_form(&mut self) -> Result<AppKey> {
        use super::ConfigMode;

        let form = &self.config_form;
        let field = |editor: &crate::tui::widgets::TextEditor| editor.content().trim().to_string();
        let (project, app_name, app_type, path) = (
            field(&form.project_name),
            field(&form.app_name),
            field(&form.app_type),
            field(&form.path),
        );
        for (value, label) in [
            (&project, "Project name"),
            (&app_name, "App name"),
            (&app_type, "App type"),
            (&path, "Path"),
        ] {
            if value.is_empty() {
                bail!("{label} is required");
            }
        }
        let original = form
            .editing
            .clone()
            .ok_or_else(|| anyhow!("No app is being edited"))?;
        let target = AppKey::new(project, app_name);

        let mut config = load_config()?;
        let mut app = config
            .projects
            .get(&original.project)
            .and_then(|p| p.apps.get(&original.app))
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "{}/{} is no longer in the config",
                    original.project,
                    original.app
                )
            })?;

        app.app_type = app_type;
        app.path = path;
        set_form_command(
            &mut app,
            Environment::Local,
            form.local_cmd_name.as_deref(),
            form.local_start_cmd.content().trim(),
        );
        set_form_command(
            &mut app,
            Environment::Docker,
            form.docker_cmd_name.as_deref(),
            form.docker_start_cmd.content().trim(),
        );

        if target == original {
            if let Some(slot) = config
                .projects
                .get_mut(&original.project)
                .and_then(|p| p.apps.get_mut(&original.app))
            {
                *slot = app;
            }
        } else {
            move_app(&mut config, &original, &target, app)?;
        }
        save_config(&config)?;
        self.config_mode = ConfigMode::View;
        Ok(target)
    }

    /// Reloads the state from the config file
    pub(crate) fn reload_state_from_config(
        &self,
        state: &mut std::sync::MutexGuard<AppState>,
    ) -> Result<()> {
        let config = load_config()?;
        state.refresh_from_config(&config)?;

        Ok(())
    }

    /// Saves a new command to an existing app
    pub(crate) fn save_new_command(&mut self, project: &str, app_name: &str) -> Result<()> {
        let project = configured_project(project);
        use super::ConfigMode;

        let command_name = self
            .config_form
            .edit_command_name
            .content()
            .trim()
            .to_string();
        let command_value = self
            .config_form
            .edit_command_value
            .content()
            .trim()
            .to_string();
        if command_name.is_empty() {
            bail!("Command name is required");
        }
        if command_value.is_empty() {
            bail!("Command is required");
        }
        let env = Environment::from_string(self.config_form.edit_command_env.as_str())
            .ok_or_else(|| anyhow!("Unknown environment {}", self.config_form.edit_command_env))?;

        let mut config = load_config()?;
        let app = config
            .projects
            .get_mut(project)
            .and_then(|p| p.apps.get_mut(app_name))
            .ok_or_else(|| anyhow!("{project}/{app_name} is not in the config"))?;
        let commands = app.commands.entry(env);
        if commands.contains_key(&command_name) {
            bail!(
                "{} already has a {} command '{command_name}' (e edits it)",
                app_name,
                env.as_str()
            );
        }
        let is_first = commands.is_empty();
        commands.insert(command_name.clone(), command_value);
        if is_first {
            *app.defaults.slot_mut(env) = Some(command_name);
        }
        save_config(&config)?;

        self.config_mode = ConfigMode::View;
        Ok(())
    }

    /// Saves a command edit.
    ///
    /// A new name renames the command (and the environment's default when it
    /// was this command); it used to add a copy under the new name and leave
    /// the old one, still the default.
    pub(crate) fn save_command_edit(&mut self, project: &str, app_name: &str) -> Result<()> {
        let project = configured_project(project);
        use super::ConfigMode;

        let new_name = self
            .config_form
            .edit_command_name
            .content()
            .trim()
            .to_string();
        let value = self
            .config_form
            .edit_command_value
            .content()
            .trim()
            .to_string();
        if new_name.is_empty() {
            bail!("Command name is required");
        }
        if value.is_empty() {
            bail!("Command is required");
        }
        let env = Environment::from_string(self.config_form.edit_command_env.as_str())
            .ok_or_else(|| anyhow!("Unknown environment {}", self.config_form.edit_command_env))?;
        let old_name = self
            .config_form
            .edit_command_original
            .clone()
            .unwrap_or_else(|| new_name.clone());

        let mut config = load_config()?;
        let app = config
            .projects
            .get_mut(project)
            .and_then(|p| p.apps.get_mut(app_name))
            .ok_or_else(|| anyhow!("{project}/{app_name} is not in the config"))?;
        let commands = app.commands.entry(env);
        if new_name != old_name {
            if commands.contains_key(&new_name) {
                bail!(
                    "{} already has a {} command '{new_name}'",
                    app_name,
                    env.as_str()
                );
            }
            commands.remove(&old_name);
            let default = app.defaults.slot_mut(env);
            if default.as_deref() == Some(old_name.as_str()) {
                *default = Some(new_name.clone());
            }
        }
        app.commands.entry(env).insert(new_name, value);
        save_config(&config)?;

        self.config_mode = ConfigMode::View;
        Ok(())
    }

    /// Deletes an app from the config
    pub(crate) fn delete_app(&self, project: &str, app: &str) -> Result<()> {
        let project = configured_project(project);
        let mut config = load_config()?;

        if let Some(proj) = config.projects.get_mut(project) {
            proj.apps.shift_remove(app);

            if proj.apps.is_empty() {
                config.projects.shift_remove(project);
            }
        }

        save_config(&config)?;
        Ok(())
    }

    /// Makes command `command_name` the default of environment `env`.
    ///
    /// The caller names the command as the panel shows it (see
    /// [`Self::get_selected_config_command`]); this used to walk the local,
    /// docker and k8s commands only, so with orbstack or docker-compose
    /// commands listed the index pointed at another command, in another
    /// environment.
    pub(crate) fn set_command_as_default(
        &self,
        project: &str,
        app_name: &str,
        env: &str,
        command_name: &str,
    ) -> Result<()> {
        let project = configured_project(project);
        let env =
            Environment::from_string(env).ok_or_else(|| anyhow!("Unknown environment {env}"))?;
        let mut config = load_config()?;
        let app = config
            .projects
            .get_mut(project)
            .and_then(|p| p.apps.get_mut(app_name))
            .ok_or_else(|| anyhow!("{project}/{app_name} is not in the config"))?;
        if !app
            .commands
            .get(env.as_str())
            .is_some_and(|commands| commands.contains_key(command_name))
        {
            bail!(
                "{app_name} has no {} command '{command_name}'",
                env.as_str()
            );
        }
        *app.defaults.slot_mut(env) = Some(command_name.to_string());
        save_config(&config)?;
        Ok(())
    }

    /// Removes a dependency from an app
    pub(crate) fn remove_dependency(
        &mut self,
        project: &str,
        app_name: &str,
        dep_idx: usize,
    ) -> Result<()> {
        let project = configured_project(project);
        let mut config = load_config()?;

        let mut new_len = 0;
        if let Some(proj) = config.projects.get_mut(project)
            && let Some(app) = proj.apps.get_mut(app_name)
            && dep_idx < app.dependencies.len()
        {
            app.dependencies.remove(dep_idx);
            new_len = app.dependencies.len();
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
        let project = configured_project(project);
        use super::ConfigMode;

        let mut config = load_config()?;

        if let Some(proj) = config.projects.get_mut(project)
            && let Some(app) = proj.apps.get_mut(app_name)
        {
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
        let project = configured_project(project);
        let mut config = load_config()?;

        if let Some(proj) = config.projects.get_mut(project)
            && let Some(app) = proj.apps.get_mut(app_name)
        {
            if let Some(env) = crate::config::models::Environment::from_string(env) {
                let slot = app.commands.slot_mut(env);
                if let Some(commands) = slot.as_mut() {
                    commands.remove(command_name);
                    if commands.is_empty() {
                        *slot = None;
                    }
                }
            }

            save_config(&config)?;
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

        if let Some(app) = state.selected_app()
            && let Some(env_files) = &app.env_files
        {
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
        Ok(())
    }

    /// Prepares deletion of the selected environment file entry
    pub(crate) fn confirm_delete_env_file(
        &mut self,
        state: &mut std::sync::MutexGuard<AppState>,
    ) -> Result<()> {
        use super::{ConfigMode, DeleteType};

        if let Some(app) = state.selected_app()
            && let Some(env_files) = &app.env_files
        {
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
        Ok(())
    }
}

/// The project in `config.json` behind a TUI row: a git worktree instance
/// (`proj@slug`) is configured by its project `proj`.
fn configured_project(project: &str) -> &str {
    // A project of the config whose name contains `@` is itself.
    let configured = crate::config::loader::load_config_cached()
        .is_ok_and(|config| config.projects.contains_key(project));
    if configured {
        project
    } else {
        crate::worktrees::split(project).0
    }
}

/// The command of `env` the "Edit app" form edits: the environment's
/// default, else its first command by name. `(name, command)`.
fn editable_command(app: &App, env: Environment) -> Option<(String, String)> {
    let commands = app.commands.get(env.as_str())?;
    app.defaults
        .get(env.as_str())
        .and_then(|name| commands.get_key_value(name))
        .or_else(|| commands.iter().min_by_key(|(name, _)| name.as_str()))
        .map(|(name, cmd)| (name.clone(), cmd.clone()))
}

/// Apply the form's command of `env` to `app`: command `name` (or a new
/// `start` one) gets `value`; an emptied field removes the command.
fn set_form_command(app: &mut App, env: Environment, name: Option<&str>, value: &str) {
    if value.is_empty() {
        let Some(name) = name else { return };
        let slot = app.commands.slot_mut(env);
        if let Some(commands) = slot.as_mut() {
            commands.remove(name);
            if commands.is_empty() {
                *slot = None;
            }
        }
        let default = app.defaults.slot_mut(env);
        if default.as_deref() == Some(name) {
            *default = None;
        }
        return;
    }
    let name = name.unwrap_or("start");
    app.commands
        .entry(env)
        .insert(name.to_string(), value.to_string());
    let default = app.defaults.slot_mut(env);
    if default.is_none() {
        *default = Some(name.to_string());
    }
}

/// Move app `from` to `to` (another name and/or project), as `app`.
///
/// Refuses when `to` exists. In the same project the app keeps its place; a
/// project left empty is removed (as deleting its last app does).
/// Dependencies on the app follow it.
fn move_app(config: &mut Config, from: &AppKey, to: &AppKey, app: App) -> Result<()> {
    if config
        .projects
        .get(&to.project)
        .is_some_and(|p| p.apps.contains_key(&to.app))
    {
        bail!("{}/{} already exists", to.project, to.app);
    }
    let source = config
        .projects
        .get_mut(&from.project)
        .ok_or_else(|| anyhow!("Project {} is no longer in the config", from.project))?;
    let index = source
        .apps
        .get_index_of(&from.app)
        .ok_or_else(|| anyhow!("{}/{} is no longer in the config", from.project, from.app))?;
    source.apps.shift_remove(&from.app);
    if from.project == to.project {
        source.apps.shift_insert(index, to.app.clone(), app);
    } else {
        if source.apps.is_empty() {
            config.projects.shift_remove(&from.project);
        }
        config
            .projects
            .entry(to.project.clone())
            .or_insert_with(|| Project {
                extra: Default::default(),
                apps: Default::default(),
                alternative_name: None,
            })
            .apps
            .insert(to.app.clone(), app);
    }

    for project in config.projects.values_mut() {
        for other in project.apps.values_mut() {
            for dep in &mut other.dependencies {
                if dep.project == from.project && dep.app == from.app {
                    dep.project = to.project.clone();
                    dep.app = to.app.clone();
                }
            }
        }
    }
    Ok(())
}

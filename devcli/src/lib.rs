pub mod cli;

pub use cli::{Cli, Commands};

use cli::{ConfigAction, EnvAction, PrefAction};
use devcli_core::commands::{
    add_env_file, auto_add_command, config_add_command, config_edit, config_edit_command,
    config_init, config_list, config_list_commands, config_remove_command, config_set_default,
    config_show, config_validate, health_check_command, internal_spawner_command, list_env_files,
    metrics_command, monitor_command, pref_reset, pref_set, pref_show, remove_env_file,
    restart_command, run_command, set_default_stage, start_command, status_command, stop_command,
    ui_command,
};
use devcli_core::Result;
use std::io::IsTerminal;

pub async fn run(cli: Cli) -> Result<()> {
    // Configure output mode once, before dispatching. Colors are disabled for
    // `--no-color`, when `NO_COLOR` is set, or when stdout is not a TTY.
    let color =
        !cli.no_color && std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal();
    devcli_core::output::init(cli.json, color);

    match cli.command {
        Commands::Start {
            app_names,
            project,
            env,
            skip_deps,
            stage,
            detached,
        } => {
            let args = devcli_core::commands::start::StartCommandArgs {
                app_names,
                project,
                env,
                skip_deps,
                silent: false,
                stage,
                detached,
                output_tx: None,
            };
            start_command(args).await?;
        }

        Commands::Restart {
            app_name,
            project,
            env,
            skip_deps,
        } => {
            let args = devcli_core::commands::restart::RestartCommandArgs {
                app_name,
                project,
                env,
                skip_deps,
                silent: false,
                output_tx: None,
            };
            restart_command(args).await?;
        }

        Commands::Stop {
            app_name,
            project,
            all,
            force,
            dry_run,
        } => {
            let args = devcli_core::commands::stop::StopCommandArgs {
                app_name,
                project,
                all,
                force,
                dry_run,
                silent: false,
                output_tx: None,
            };
            stop_command(args).await?;
        }

        Commands::Run {
            app_name,
            command_variant,
            project,
            env,
            skip_deps,
        } => {
            let args = devcli_core::commands::run::RunCommandArgs {
                app_name,
                command_variant,
                project,
                env,
                skip_deps,
                silent: false,
                output_tx: None,
            };
            run_command(args).await?;
        }

        Commands::Status {
            app_name,
            project,
            deps,
        } => {
            let args = devcli_core::commands::status::StatusCommandArgs {
                app_name,
                project,
                show_deps: deps,
            };
            status_command(args).await?;
        }

        Commands::AutoAdd {
            path,
            yes,
            project,
            name,
            app_type,
        } => {
            let args = devcli_core::commands::auto_add::AutoAddArgs {
                path,
                yes,
                project,
                name,
                app_type,
            };
            auto_add_command(args).await?;
        }

        Commands::Logs {
            app_name,
            project,
            env,
            lines,
            follow,
        } => {
            let args = devcli_core::commands::logs::LogsArgs {
                app_name,
                project,
                environment: env,
                lines,
                follow,
            };
            devcli_core::commands::logs_command(args).await?;
        }

        Commands::HealthCheck { app_name, env } => {
            let args = devcli_core::commands::health_check::HealthCheckArgs {
                app_name,
                environment: env,
            };
            health_check_command(args).await?;
        }

        Commands::Monitor { daemon } => {
            monitor_command(daemon).await?;
        }

        Commands::Metrics => {
            metrics_command().await?;
        }

        Commands::InternalSpawner { payload } => {
            internal_spawner_command(payload).await?;
        }

        Commands::Config { action } => match action {
            ConfigAction::Init => config_init().await?,
            ConfigAction::Validate => config_validate().await?,
            ConfigAction::List { project, apps_only } => config_list(project, apps_only).await?,
            ConfigAction::Show { app_name, project } => config_show(app_name, project).await?,
            ConfigAction::Edit => config_edit().await?,
            ConfigAction::AddCommand {
                app_name,
                environment,
                command_name,
                command_value,
                project,
            } => {
                config_add_command(app_name, project, environment, command_name, command_value)
                    .await?
            }
            ConfigAction::RemoveCommand {
                app_name,
                environment,
                command_name,
                project,
            } => config_remove_command(app_name, project, environment, command_name).await?,
            ConfigAction::SetDefault {
                app_name,
                environment,
                command_name,
                project,
            } => config_set_default(app_name, project, environment, command_name).await?,
            ConfigAction::ListCommands {
                app_name,
                project,
                env,
            } => config_list_commands(app_name, project, env).await?,
            ConfigAction::EditCommand {
                app_name,
                environment,
                command_name,
                project,
            } => config_edit_command(app_name, project, environment, command_name).await?,
        },

        Commands::Env { action } => match action {
            EnvAction::Add {
                app_name,
                stage,
                context,
                file,
            } => add_env_file(&app_name, &stage, &context, &file)?,
            EnvAction::Remove {
                app_name,
                stage,
                context,
            } => remove_env_file(&app_name, &stage, context.as_deref())?,
            EnvAction::List { app_name } => list_env_files(&app_name)?,
            EnvAction::SetDefault {
                app_name,
                context,
                stage,
            } => set_default_stage(&app_name, &context, &stage)?,
        },

        Commands::Pref { action } => match action {
            PrefAction::Set { key, value } => pref_set(key, value).await?,
            PrefAction::Show => pref_show().await?,
            PrefAction::Reset => pref_reset().await?,
        },

        Commands::Ui => {
            ui_command().await?;
        }
    }

    Ok(())
}

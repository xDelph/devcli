use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "devcli")]
#[command(about = "A powerful CLI for managing spawned processes", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum Commands {
    #[command(about = "Start a process using config")]
    Start {
        #[arg(help = "Name of the application/process from config (can specify multiple)")]
        app_names: Vec<String>,

        #[arg(short, long, help = "Project name (required if app name is ambiguous)")]
        project: Option<String>,

        #[arg(
            short,
            long,
            help = "Environment: 'local', 'docker', or 'k8s' (overrides preference)"
        )]
        env: Option<String>,

        #[arg(long, help = "Skip dependency checks")]
        skip_deps: bool,

        #[arg(
            short,
            long,
            help = "Deployment stage: 'dev', 'qa', 'preprod', or 'prod' (overrides config)"
        )]
        stage: Option<String>,
    },

    #[command(about = "Restart a running process using same config")]
    Restart {
        #[arg(help = "Name of the application/process to restart")]
        app_name: String,

        #[arg(short, long, help = "Project name (required if app name is ambiguous)")]
        project: Option<String>,

        #[arg(
            short,
            long,
            help = "Environment: 'local', 'docker', or 'k8s' (overrides existing config)"
        )]
        env: Option<String>,

        #[arg(long, help = "Skip dependency checks")]
        skip_deps: bool,
    },

    #[command(about = "Stop one or more running processes")]
    Stop {
        #[arg(help = "Optional: specific app name to stop")]
        app_name: Option<String>,

        #[arg(short, long, help = "Stop all apps in a specific project")]
        project: Option<String>,

        #[arg(long, help = "Stop all running processes")]
        all: bool,

        #[arg(long, help = "Force kill processes (use SIGKILL instead of SIGTERM)")]
        force: bool,
    },

    #[command(about = "Run a specific command variant for an app")]
    Run {
        #[arg(help = "Name of the application/process from config")]
        app_name: String,

        #[arg(help = "Command variant to run (e.g., 'build:production', 'test:apps')")]
        command_variant: String,

        #[arg(short, long, help = "Project name (required if app name is ambiguous)")]
        project: Option<String>,

        #[arg(
            short,
            long,
            help = "Environment: 'local', 'docker', or 'k8s' (overrides preference)"
        )]
        env: Option<String>,

        #[arg(long, help = "Skip dependency checks")]
        skip_deps: bool,
    },

    #[command(about = "Show status of running processes")]
    Status {
        #[arg(help = "Optional: specific app name to check")]
        app_name: Option<String>,

        #[arg(short, long, help = "Filter by project name")]
        project: Option<String>,

        #[arg(long, help = "Show dependency status")]
        deps: bool,
    },

    #[command(about = "Auto-detect and add app to config")]
    AutoAdd {
        #[arg(long, help = "Path to detect (defaults to current directory)")]
        path: Option<String>,
    },

    #[command(about = "Manually check the health of a running app")]
    HealthCheck {
        #[arg(help = "Name of the application to check")]
        app_name: String,

        #[arg(
            short,
            long,
            help = "Environment: 'local', 'docker', 'orbstack', or 'k8s'"
        )]
        env: Option<String>,
    },

    #[command(about = "Monitor process health (internal use)")]
    Monitor {
        #[arg(long, help = "Run as background daemon")]
        daemon: bool,
    },

    #[command(about = "Display metrics from the monitor daemon")]
    Metrics,

    #[command(hide = true)]
    InternalSpawner {
        #[arg(long, help = "Base64-encoded JSON payload")]
        payload: String,
    },

    #[command(about = "Manage configuration")]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    #[command(about = "Manage environment files")]
    Env {
        #[command(subcommand)]
        action: EnvAction,
    },

    #[command(about = "Manage preferences")]
    Pref {
        #[command(subcommand)]
        action: PrefAction,
    },

    #[command(about = "Launch interactive TUI")]
    Ui,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum ConfigAction {
    #[command(about = "Initialize a new config file")]
    Init,

    #[command(about = "Validate the config file")]
    Validate,

    #[command(about = "List all projects and apps")]
    List {
        #[arg(short, long, help = "Filter by project name")]
        project: Option<String>,

        #[arg(long, help = "Show only app names")]
        apps_only: bool,
    },

    #[command(about = "Show details of a specific app")]
    Show {
        #[arg(help = "App name to show")]
        app_name: String,

        #[arg(short, long, help = "Project name (required if app name is ambiguous)")]
        project: Option<String>,
    },

    #[command(about = "Edit the config file")]
    Edit,

    #[command(about = "Add a command to an app")]
    AddCommand {
        #[arg(help = "App name (optional - will prompt if not provided)")]
        app_name: Option<String>,

        #[arg(help = "Environment (local, docker, k8s) (optional - will prompt if not provided)")]
        environment: Option<String>,

        #[arg(help = "Command name (optional - will prompt if not provided)")]
        command_name: Option<String>,

        #[arg(help = "Command value (optional - will prompt if not provided)")]
        command_value: Option<String>,

        #[arg(short, long, help = "Project name (required if app name is ambiguous)")]
        project: Option<String>,
    },

    #[command(about = "Remove a command from an app")]
    RemoveCommand {
        #[arg(help = "App name (optional - will prompt if not provided)")]
        app_name: Option<String>,

        #[arg(help = "Environment (local, docker, k8s) (optional - will prompt if not provided)")]
        environment: Option<String>,

        #[arg(help = "Command name (optional - will prompt if not provided)")]
        command_name: Option<String>,

        #[arg(short, long, help = "Project name (required if app name is ambiguous)")]
        project: Option<String>,
    },

    #[command(about = "Set the default command for an environment")]
    SetDefault {
        #[arg(help = "App name (optional - will prompt if not provided)")]
        app_name: Option<String>,

        #[arg(help = "Environment (local, docker, k8s) (optional - will prompt if not provided)")]
        environment: Option<String>,

        #[arg(help = "Command name (optional - will prompt if not provided)")]
        command_name: Option<String>,

        #[arg(short, long, help = "Project name (required if app name is ambiguous)")]
        project: Option<String>,
    },

    #[command(about = "List all commands for an app")]
    ListCommands {
        #[arg(help = "App name (optional - will prompt if not provided)")]
        app_name: Option<String>,

        #[arg(short, long, help = "Project name (required if app name is ambiguous)")]
        project: Option<String>,

        #[arg(short, long, help = "Filter by environment (local, docker, k8s)")]
        env: Option<String>,
    },

    #[command(about = "Edit a specific command for an app")]
    EditCommand {
        #[arg(help = "App name (optional - will prompt if not provided)")]
        app_name: Option<String>,

        #[arg(help = "Environment (local, docker, k8s) (optional - will prompt if not provided)")]
        environment: Option<String>,

        #[arg(help = "Command name (optional - will prompt if not provided)")]
        command_name: Option<String>,

        #[arg(short, long, help = "Project name (required if app name is ambiguous)")]
        project: Option<String>,
    },
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum EnvAction {
    #[command(about = "Add an environment file to an app")]
    Add {
        #[arg(help = "App name")]
        app_name: String,

        #[arg(short, long, help = "Stage name (e.g., 'qa', 'prod', 'dev')")]
        stage: String,

        #[arg(
            short,
            long,
            help = "Context name (e.g., 'local', 'docker', 'orbstack', 'k8s')"
        )]
        context: String,

        #[arg(short, long, help = "Path to env file relative to app root")]
        file: String,
    },

    #[command(about = "Remove an environment file or stage from an app")]
    Remove {
        #[arg(help = "App name")]
        app_name: String,

        #[arg(short, long, help = "Stage name to remove")]
        stage: String,

        #[arg(short, long, help = "Context name (if omitted, removes entire stage)")]
        context: Option<String>,
    },

    #[command(about = "List environment files for an app")]
    List {
        #[arg(help = "App name")]
        app_name: String,
    },

    #[command(about = "Set default stage for a context")]
    SetDefault {
        #[arg(help = "App name")]
        app_name: String,

        #[arg(
            short,
            long,
            help = "Context name (e.g., 'local', 'docker', 'orbstack', 'k8s')"
        )]
        context: String,

        #[arg(short, long, help = "Stage name to set as default")]
        stage: String,
    },
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum PrefAction {
    #[command(about = "Set a preference value")]
    Set {
        #[arg(help = "Preference key (e.g., 'default-env')")]
        key: String,

        #[arg(help = "Preference value")]
        value: String,
    },

    #[command(about = "Show current preferences")]
    Show,

    #[command(about = "Reset preferences to defaults")]
    Reset,
}

/// TUI mode must not write tracing logs to stderr (ratatui corruption).
pub fn console_tracing_enabled(command: &Commands) -> bool {
    !matches!(command, Commands::Ui)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_start_with_multiple_apps_and_flags() {
        let cli = Cli::try_parse_from([
            "devcli",
            "start",
            "api",
            "worker",
            "--project",
            "qm",
            "--env",
            "docker",
            "--skip-deps",
            "--stage",
            "qa",
        ])
        .expect("parse start");

        assert_eq!(
            cli.command,
            Commands::Start {
                app_names: vec!["api".into(), "worker".into()],
                project: Some("qm".into()),
                env: Some("docker".into()),
                skip_deps: true,
                stage: Some("qa".into()),
            }
        );
    }

    #[test]
    fn parse_restart_and_run_variants() {
        let restart = Cli::try_parse_from([
            "devcli",
            "restart",
            "api",
            "-p",
            "qm",
            "-e",
            "local",
            "--skip-deps",
        ])
        .expect("parse restart")
        .command;

        assert_eq!(
            restart,
            Commands::Restart {
                app_name: "api".into(),
                project: Some("qm".into()),
                env: Some("local".into()),
                skip_deps: true,
            }
        );

        let run = Cli::try_parse_from([
            "devcli",
            "run",
            "api",
            "build:production",
            "--project",
            "qm",
        ])
        .expect("parse run")
        .command;

        assert_eq!(
            run,
            Commands::Run {
                app_name: "api".into(),
                command_variant: "build:production".into(),
                project: Some("qm".into()),
                env: None,
                skip_deps: false,
            }
        );
    }

    #[test]
    fn parse_stop_status_and_auto_add() {
        let stop = Cli::try_parse_from(["devcli", "stop", "--all", "--force"])
            .expect("parse stop")
            .command;
        assert_eq!(
            stop,
            Commands::Stop {
                app_name: None,
                project: None,
                all: true,
                force: true,
            }
        );

        let status = Cli::try_parse_from(["devcli", "status", "api", "-p", "qm", "--deps"])
            .expect("parse status")
            .command;
        assert_eq!(
            status,
            Commands::Status {
                app_name: Some("api".into()),
                project: Some("qm".into()),
                deps: true,
            }
        );

        let auto_add = Cli::try_parse_from(["devcli", "auto-add", "--path", "/tmp/app"])
            .expect("parse auto-add")
            .command;
        assert_eq!(
            auto_add,
            Commands::AutoAdd {
                path: Some("/tmp/app".into()),
            }
        );
    }

    #[test]
    fn parse_internal_monitor_and_metrics_commands() {
        let health = Cli::try_parse_from(["devcli", "health-check", "api", "--env", "orbstack"])
            .expect("parse health-check")
            .command;
        assert_eq!(
            health,
            Commands::HealthCheck {
                app_name: "api".into(),
                env: Some("orbstack".into()),
            }
        );

        let monitor = Cli::try_parse_from(["devcli", "monitor", "--daemon"])
            .expect("parse monitor")
            .command;
        assert_eq!(monitor, Commands::Monitor { daemon: true });

        assert!(matches!(
            Cli::try_parse_from(["devcli", "metrics"])
                .expect("parse metrics")
                .command,
            Commands::Metrics
        ));

        let spawner = Cli::try_parse_from([
            "devcli",
            "internal-spawner",
            "--payload",
            "dGVzdA==",
        ])
        .expect("parse internal-spawner")
        .command;
        assert_eq!(
            spawner,
            Commands::InternalSpawner {
                payload: "dGVzdA==".into(),
            }
        );
    }

    #[test]
    fn parse_config_env_and_pref_subcommands() {
        let config = Cli::try_parse_from(["devcli", "config", "list", "-p", "qm", "--apps-only"])
            .expect("parse config list")
            .command;
        assert_eq!(
            config,
            Commands::Config {
                action: ConfigAction::List {
                    project: Some("qm".into()),
                    apps_only: true,
                }
            }
        );

        let env = Cli::try_parse_from([
            "devcli",
            "env",
            "add",
            "api",
            "--stage",
            "qa",
            "--context",
            "local",
            "--file",
            ".env.qa",
        ])
        .expect("parse env add")
        .command;
        assert_eq!(
            env,
            Commands::Env {
                action: EnvAction::Add {
                    app_name: "api".into(),
                    stage: "qa".into(),
                    context: "local".into(),
                    file: ".env.qa".into(),
                }
            }
        );

        let pref = Cli::try_parse_from(["devcli", "pref", "set", "default-env", "docker"])
            .expect("parse pref set")
            .command;
        assert_eq!(
            pref,
            Commands::Pref {
                action: PrefAction::Set {
                    key: "default-env".into(),
                    value: "docker".into(),
                }
            }
        );
    }

    #[test]
    fn parse_ui_command() {
        assert!(matches!(
            Cli::try_parse_from(["devcli", "ui"])
                .expect("parse ui")
                .command,
            Commands::Ui
        ));
    }

    #[test]
    fn missing_subcommand_fails() {
        assert!(Cli::try_parse_from(["devcli"]).is_err());
    }

    #[test]
    fn unknown_subcommand_fails() {
        assert!(Cli::try_parse_from(["devcli", "not-a-command"]).is_err());
    }

    #[test]
    fn console_tracing_disabled_for_ui_only() {
        assert!(!console_tracing_enabled(&Commands::Ui));
        assert!(console_tracing_enabled(&Commands::Metrics));
        assert!(console_tracing_enabled(&Commands::Start {
            app_names: vec!["api".into()],
            project: None,
            env: None,
            skip_deps: false,
            stage: None,
        }));
    }
}

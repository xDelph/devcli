// This is the entry point for the devcli binary
// It handles CLI argument parsing and routes to the appropriate command
//
// Architecture:
// main() → parse CLI args → match command → call command function

// Import the clap library for CLI argument parsing
// Parser and Subcommand are "derive macros" - they generate code for us
use clap::{Parser, Subcommand};

// Import our command implementations from the core library
use devcli_core::commands::{
    add_env_file, auto_add_command, config_add_command, config_edit, config_edit_command,
    config_init, config_list, config_list_commands, config_remove_command, config_set_default,
    config_show, config_validate, health_check_command, internal_spawner_command, list_env_files,
    metrics_command, monitor_command, pref_reset, pref_set, pref_show, remove_env_file,
    restart_command, run_command, set_default_stage, start_command, status_command, stop_command,
    ui_command,
};
use devcli_core::logging::init_tracing;
use devcli_core::Result; // Our error handling type

// Main CLI structure
// #[derive(Parser)] automatically implements argument parsing
// #[command(...)] configures the CLI metadata
#[derive(Parser)]
#[command(name = "devcli")]
#[command(about = "A powerful CLI for managing spawned processes", long_about = None)]
#[command(version)] // Automatically adds --version flag
struct Cli {
    // The command to run (start, run, status, config, pref)
    #[command(subcommand)]
    command: Commands,
}

// Define the available subcommands
// enum in Rust is like a "union" - it can be one of several variants
// Each variant can have its own fields (like a struct)
#[derive(Subcommand)]
enum Commands {
    // The "start" subcommand
    // Example: devcli start api-private --env docker
    #[command(about = "Start a process using config")]
    Start {
        // Each field becomes a CLI argument
        // #[arg(...)] configures how it's parsed
        #[arg(help = "Name of the application/process from config (can specify multiple)")]
        app_names: Vec<String>, // Multiple app names can be provided

        #[arg(short, long, help = "Project name (required if app name is ambiguous)")]
        project: Option<String>, // Optional flag: --project or -p
        // Option<String> means it might be Some("value") or None
        #[arg(
            short,
            long,
            help = "Environment: 'local', 'docker', or 'k8s' (overrides preference)"
        )]
        env: Option<String>, // Optional flag: --env or -e

        #[arg(long, help = "Skip dependency checks")]
        skip_deps: bool, // Boolean flag: --skip-deps (no value needed)

        #[arg(
            short,
            long,
            help = "Deployment stage: 'dev', 'qa', 'preprod', or 'prod' (overrides config)"
        )]
        stage: Option<String>, // Optional flag: --stage or -s
    },

    // The "restart" subcommand
    // Example: devcli restart api-private
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

    // The "stop" subcommand
    // Example: devcli stop api-private
    // Example: devcli stop --all
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

    // The "run" subcommand
    // Example: devcli run api-private build:production
    #[command(about = "Run a specific command variant for an app")]
    Run {
        #[arg(help = "Name of the application/process from config")]
        app_name: String,

        #[arg(help = "Command variant to run (e.g., 'build:production', 'test:apps')")]
        command_variant: String, // Required positional argument

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

    // The "status" subcommand
    // Example: devcli status --project qm --deps
    #[command(about = "Show status of running processes")]
    Status {
        #[arg(help = "Optional: specific app name to check")]
        app_name: Option<String>, // Optional positional argument

        #[arg(short, long, help = "Filter by project name")]
        project: Option<String>,

        #[arg(long, help = "Show dependency status")]
        deps: bool, // Boolean flag: --deps
    },

    #[command(about = "Auto-detect and add app to config")]
    AutoAdd {
        #[arg(long, help = "Path to detect (defaults to current directory)")]
        path: Option<String>,
    },

    // The "health-check" subcommand
    // Example: devcli health-check api
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

    // The "monitor" subcommand
    // Example: devcli monitor --daemon (internal use, spawned automatically)
    #[command(about = "Monitor process health (internal use)")]
    Monitor {
        #[arg(long, help = "Run as background daemon")]
        daemon: bool,
    },

    // The "metrics" subcommand
    // Example: devcli metrics
    #[command(about = "Display metrics from the monitor daemon")]
    Metrics,

    // The "internal-spawner" subcommand (hidden, for internal use only)
    // Spawned by devcli start to manage app processes and log capturing
    #[command(hide = true)]
    InternalSpawner {
        #[arg(long, help = "Base64-encoded JSON payload")]
        payload: String,
    },

    // The "config" subcommand with nested subcommands
    // Example: devcli config validate
    #[command(about = "Manage configuration")]
    Config {
        #[command(subcommand)]
        action: ConfigAction, // Nested subcommands (init, validate, etc.)
    },

    // The "env" subcommand with nested subcommands
    // Example: devcli env add myapp --stage qa --context local --file .env.qa
    #[command(about = "Manage environment files")]
    Env {
        #[command(subcommand)]
        action: EnvAction, // Nested subcommands (add, remove, list, set-default)
    },

    // The "pref" subcommand with nested subcommands
    // Example: devcli pref set default-env local
    #[command(about = "Manage preferences")]
    Pref {
        #[command(subcommand)]
        action: PrefAction, // Nested subcommands (set, show, reset)
    },

    // The "ui" subcommand
    // Example: devcli ui
    #[command(about = "Launch interactive TUI")]
    Ui,
}

// Nested subcommands for "config"
// Each becomes: devcli config <action>
#[derive(Subcommand)]
enum ConfigAction {
    #[command(about = "Initialize a new config file")]
    Init, // No arguments

    #[command(about = "Validate the config file")]
    Validate, // No arguments

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
    Edit, // No arguments

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

// Nested subcommands for "env"
// Each becomes: devcli env <action>
#[derive(Subcommand)]
enum EnvAction {
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

// Nested subcommands for "pref"
// Each becomes: devcli pref <action>
#[derive(Subcommand)]
enum PrefAction {
    #[command(about = "Set a preference value")]
    Set {
        #[arg(help = "Preference key (e.g., 'default-env')")]
        key: String,

        #[arg(help = "Preference value")]
        value: String,
    },

    #[command(about = "Show current preferences")]
    Show, // No arguments

    #[command(about = "Reset preferences to defaults")]
    Reset, // No arguments
}

// Main entry point for the program
// #[tokio::main] transforms this into an async runtime
// It sets up tokio's async executor and runs the async main function
#[tokio::main]
async fn main() {
    // Call run() and handle any errors
    // 'if let Err(e)' only runs if run() returns an error
    if let Err(e) = run().await {
        // Print error to stderr (standard error output)
        eprintln!("Error: {}", e);
        // Exit with non-zero status code (indicates failure)
        std::process::exit(1);
    }
    // If run() succeeds, program exits normally with code 0
}

// Main application logic
// Separated from main() so we can use ? for error handling
async fn run() -> Result<()> {
    // Parse first so TUI mode can disable stderr tracing (it corrupts ratatui).
    let cli = Cli::parse();

    let enable_console = !matches!(cli.command, Commands::Ui);
    // JSON file logging always; stderr console only for CLI commands.
    let _ = init_tracing(enable_console);

    // Match on which command was provided
    // This is like a switch statement but more powerful
    match cli.command {
        // Handle the "start" command
        // Supports multiple app names (e.g., devcli start redis.local traefik.local)
        Commands::Start {
            app_names,
            project,
            env,
            skip_deps,
            stage,
        } => {
            // Bundle the arguments into a struct
            // This is the pattern we use: CLI args → struct → command function
            // app_names is Vec<String> and can contain multiple app names
            let args = devcli_core::commands::start::StartCommandArgs {
                app_names, // Vec<String> - can contain multiple app names
                project,
                env,
                skip_deps,
                silent: false, // CLI mode - show output to terminal
                stage,
                output_tx: None,
            };

            // Call the start command from our core library
            // .await waits for the async function to complete
            // ? returns any error immediately
            // In non-detached mode, this will keep the process alive to show logs
            start_command(args).await?;
        }

        // Handle the "restart" command
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
                silent: false, // CLI mode - show output
                output_tx: None,
            };

            restart_command(args).await?;
        }

        // Handle the "stop" command
        Commands::Stop {
            app_name,
            project,
            all,
            force,
        } => {
            let args = devcli_core::commands::stop::StopCommandArgs {
                app_name,
                project,
                all,
                force,
                silent: false, // CLI mode - show output
                output_tx: None,
            };

            stop_command(args).await?;
        }

        // Handle the "run" command
        Commands::Run {
            app_name,
            command_variant,
            project,
            env,
            skip_deps,
        } => {
            // Same pattern: bundle args into struct, call command
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

        // Handle the "status" command
        Commands::Status {
            app_name,
            project,
            deps,
        } => {
            // Bundle args and call status command
            let args = devcli_core::commands::status::StatusCommandArgs {
                app_name,
                project,
                show_deps: deps, // Note: rename deps → show_deps
            };
            status_command(args).await?;
        }

        Commands::AutoAdd { path } => {
            auto_add_command(path).await?;
        }

        // Handle the "health-check" command
        Commands::HealthCheck { app_name, env } => {
            let args = devcli_core::commands::health_check::HealthCheckArgs {
                app_name,
                environment: env,
            };
            health_check_command(args).await?;
        }

        // Handle the "monitor" command
        Commands::Monitor { daemon } => {
            monitor_command(daemon).await?;
        }

        // Handle the "metrics" command
        Commands::Metrics => {
            metrics_command().await?;
        }

        // Handle the "internal-spawner" command (hidden)
        Commands::InternalSpawner { payload } => {
            internal_spawner_command(payload).await?;
        }

        // Handle the "config" command and its subcommands
        Commands::Config { action } => match action {
            // Each nested command calls its corresponding function
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

        // Handle the "env" command and its subcommands
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

        // Handle the "pref" command and its subcommands
        Commands::Pref { action } => match action {
            PrefAction::Set { key, value } => pref_set(key, value).await?,
            PrefAction::Show => pref_show().await?,
            PrefAction::Reset => pref_reset().await?,
        },

        // Handle the "ui" command
        Commands::Ui => {
            ui_command().await?;
        }
    }

    // Return Ok(()) to indicate success
    Ok(())
}

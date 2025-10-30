// This is the entry point for the rustycli binary
// It handles CLI argument parsing and routes to the appropriate command
//
// Architecture:
// main() → parse CLI args → match command → call command function

// Import the clap library for CLI argument parsing
// Parser and Subcommand are "derive macros" - they generate code for us
use clap::{Parser, Subcommand};

// Import our command implementations from the core library
use rustycli_core::commands::{
    auto_add_command, config_edit, config_init, config_list, config_show, config_validate,
    monitor_command, pref_reset, pref_set, pref_show, run_command, start_command, status_command,
};
use rustycli_core::Result; // Our error handling type

// Main CLI structure
// #[derive(Parser)] automatically implements argument parsing
// #[command(...)] configures the CLI metadata
#[derive(Parser)]
#[command(name = "rustycli")]
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
    // Example: rustycli start api-private --env docker
    #[command(about = "Start a process using config")]
    Start {
        // Each field becomes a CLI argument
        // #[arg(...)] configures how it's parsed
        
        #[arg(help = "Name of the application/process from config")]
        app_name: String, // Required positional argument
        
        #[arg(short, long, help = "Project name (required if app name is ambiguous)")]
        project: Option<String>, // Optional flag: --project or -p
        // Option<String> means it might be Some("value") or None
        
        #[arg(short, long, help = "Environment: 'local', 'docker', or 'k8s' (overrides preference)")]
        env: Option<String>, // Optional flag: --env or -e
        
        #[arg(long, help = "Skip dependency checks")]
        skip_deps: bool, // Boolean flag: --skip-deps (no value needed)
    },
    
    // The "run" subcommand
    // Example: rustycli run api-private build:production
    #[command(about = "Run a specific command variant for an app")]
    Run {
        #[arg(help = "Name of the application/process from config")]
        app_name: String,
        
        #[arg(help = "Command variant to run (e.g., 'build:production', 'test:apps')")]
        command_variant: String, // Required positional argument
        
        #[arg(short, long, help = "Project name (required if app name is ambiguous)")]
        project: Option<String>,
        
        #[arg(short, long, help = "Environment: 'local', 'docker', or 'k8s' (overrides preference)")]
        env: Option<String>,
        
        #[arg(long, help = "Skip dependency checks")]
        skip_deps: bool,
    },
    
    // The "status" subcommand
    // Example: rustycli status --project qm --deps
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
    
    // The "monitor" subcommand
    // Example: rustycli monitor --daemon (internal use, spawned automatically)
    #[command(about = "Monitor process health (internal use)")]
    Monitor {
        #[arg(long, help = "Run as background daemon")]
        daemon: bool,
    },
    
    // The "config" subcommand with nested subcommands
    // Example: rustycli config validate
    #[command(about = "Manage configuration")]
    Config {
        #[command(subcommand)]
        action: ConfigAction, // Nested subcommands (init, validate, etc.)
    },
    
    // The "pref" subcommand with nested subcommands
    // Example: rustycli pref set default-env local
    #[command(about = "Manage preferences")]
    Pref {
        #[command(subcommand)]
        action: PrefAction, // Nested subcommands (set, show, reset)
    },
}

// Nested subcommands for "config"
// Each becomes: rustycli config <action>
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
}

// Nested subcommands for "pref"
// Each becomes: rustycli pref <action>
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
    // Parse command-line arguments into our Cli struct
    // This automatically handles --help, --version, validation, etc.
    let cli = Cli::parse();
    
    // Match on which command was provided
    // This is like a switch statement but more powerful
    match cli.command {
        // Handle the "start" command
        Commands::Start {
            app_name,
            project,
            env,
            skip_deps,
        } => {
            // Bundle the arguments into a struct
            // This is the pattern we use: CLI args → struct → command function
            let args = rustycli_core::commands::start::StartCommandArgs {
                app_name,
                project,
                env,
                skip_deps,
            };
            
            // Call the start command from our core library
            // .await waits for the async function to complete
            // ? returns any error immediately
            start_command(args).await?;
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
            let args = rustycli_core::commands::run::RunCommandArgs {
                app_name,
                command_variant,
                project,
                env,
                skip_deps,
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
            let args = rustycli_core::commands::status::StatusCommandArgs {
                app_name,
                project,
                show_deps: deps, // Note: rename deps → show_deps
            };
            status_command(args).await?;
        }
        
        Commands::AutoAdd { path } => {
            auto_add_command(path).await?;
        }
        
        // Handle the "monitor" command
        Commands::Monitor { daemon } => {
            monitor_command(daemon).await?;
        }
        
        // Handle the "config" command and its subcommands
        Commands::Config { action } => match action {
            // Each nested command calls its corresponding function
            ConfigAction::Init => config_init().await?,
            ConfigAction::Validate => config_validate().await?,
            ConfigAction::List { project, apps_only } => config_list(project, apps_only).await?,
            ConfigAction::Show { app_name, project } => config_show(app_name, project).await?,
            ConfigAction::Edit => config_edit().await?,
        },
        
        // Handle the "pref" command and its subcommands
        Commands::Pref { action } => match action {
            PrefAction::Set { key, value } => pref_set(key, value).await?,
            PrefAction::Show => pref_show().await?,
            PrefAction::Reset => pref_reset().await?,
        },
    }
    
    // Return Ok(()) to indicate success
    Ok(())
}

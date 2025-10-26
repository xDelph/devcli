// This is the entry point for the rustycli binary
// It handles CLI argument parsing and routes to the appropriate command

// Import the clap library for CLI argument parsing
// Parser and Subcommand are "derive macros" - they generate code for us
use clap::{Parser, Subcommand};
// Import our command implementations from the core library
use rustycli_core::commands::{start_command, status_command};
use rustycli_core::Result; // Our error handling type
use std::collections::HashMap; // For environment variables
use std::path::PathBuf; // For file paths

// Main CLI structure
// #[derive(Parser)] automatically implements argument parsing
// #[command(...)] configures the CLI metadata
#[derive(Parser)]
#[command(name = "rustycli")]
#[command(about = "A powerful CLI for managing spawned processes", long_about = None)]
#[command(version)] // Automatically adds --version flag
struct Cli {
    // The command to run (start, status, etc.)
    #[command(subcommand)]
    command: Commands,
}

// Define the available subcommands
// enum in Rust is like a "union" - it can be one of several variants
#[derive(Subcommand)]
enum Commands {
    // The "start" subcommand
    #[command(about = "Start a new process")]
    Start {
        // Each field becomes a CLI argument
        // #[arg(...)] configures how it's parsed
        #[arg(help = "Name of the application/process")]
        app_name: String, // Required positional argument

        #[arg(short = 'd', long, help = "Working directory for the process")]
        dir: Option<PathBuf>, // Optional flag: --dir or -d
        // Option<T> means it might be Some(value) or None
        #[arg(short = 'c', long, help = "Command to execute")]
        cmd: String, // Required flag: --cmd or -c

        #[arg(short = 'e', long, help = "Environment variables (KEY=VALUE)", value_parser = parse_key_val)]
        env: Vec<(String, String)>, // Can be specified multiple times
        // Vec<(String, String)> is a list of (key, value) tuples
        #[arg(long, help = "Run process in detached mode")]
        detach: bool, // Boolean flag: --detach (no value needed)
    },

    // The "status" subcommand
    #[command(about = "Show status of running processes")]
    Status {
        #[arg(help = "Optional: specific app name to check")]
        app_name: Option<String>, // Optional positional argument
    },
}

// Helper function to parse KEY=VALUE strings
// Used by the --env flag
// &str is a string slice (borrowed reference to a string)
fn parse_key_val(s: &str) -> Result<(String, String)> {
    // Split on '=' character, limit to 2 parts
    // This handles cases like "KEY=VALUE=WITH=EQUALS"
    let parts: Vec<&str> = s.splitn(2, '=').collect();

    // Validate we got exactly 2 parts
    if parts.len() != 2 {
        // If not, return an error
        anyhow::bail!("Environment variable must be in KEY=VALUE format");
    }

    // Convert &str to String and return as tuple
    // (key, value)
    Ok((parts[0].to_string(), parts[1].to_string()))
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
        Commands::Start {
            app_name,
            dir,
            cmd,
            env,
            detach,
        } => {
            // If no directory was provided, use current directory
            // unwrap_or_else takes a closure (anonymous function) for lazy evaluation
            let working_dir = dir.unwrap_or_else(|| {
                // Get current directory or panic if it fails
                // expect() is like unwrap() but with a custom error message
                std::env::current_dir().expect("Failed to get current directory")
            });

            // Convert Vec<(String, String)> to HashMap<String, String>
            // into_iter() consumes the Vec (takes ownership)
            // .collect() gathers the pairs into a HashMap
            let env_vars: HashMap<String, String> = env.into_iter().collect();

            // Build the arguments struct for start_command
            let args = rustycli_core::commands::start::StartCommandArgs {
                app_name,
                working_dir,
                command: cmd, // 'cmd' from CLI becomes 'command' in struct
                env_vars,
                detached: detach,
            };

            // Call the start command from our core library
            // .await waits for the async function to complete
            // ? returns any error immediately
            start_command(args).await?;
        }

        Commands::Status { app_name } => {
            // Call the status command
            // app_name is already Option<String>, so we just pass it
            status_command(app_name).await?;
        }
    }

    // Return Ok(()) to indicate success
    Ok(())
}

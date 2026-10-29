use clap::Parser;
use devcli::{cli, run, Cli};
use devcli_core::logging::init_tracing;

#[tokio::main]
async fn main() {
    match run_main().await {
        Ok(()) => {
            // Commands may report a result via an exit code (status, health-check)
            // without treating it as an error.
            let code = devcli_core::output::exit_code();
            if code != 0 {
                std::process::exit(code);
            }
        }
        Err(e) => {
            if devcli_core::output::json_enabled() {
                devcli_core::output::print_json_error(&e.to_string());
            } else {
                eprintln!("Error: {}", e);
            }
            std::process::exit(1);
        }
    }
}

async fn run_main() -> devcli_core::Result<()> {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => exit_on_parse_error(err),
    };
    // `--json` implies machine-readable stdout; keep stderr free of log noise too.
    let enable_console = cli::console_tracing_enabled(&cli.command) && !cli.json;
    let _ = init_tracing(enable_console);
    run(cli).await
}

/// Report a command-line parse error.
///
/// With `--json` anywhere on the command line, a usage error follows the same
/// contract as every other failure (`{"error": ...}` on stderr, exit 1) instead
/// of clap's plain-text usage and exit 2. `--help` and `--version` are not
/// errors and keep clap's normal output.
fn exit_on_parse_error(err: clap::Error) -> ! {
    use clap::error::ErrorKind;

    let wants_json = std::env::args_os().any(|a| a == "--json");
    let informational = matches!(
        err.kind(),
        ErrorKind::DisplayHelp
            | ErrorKind::DisplayVersion
            | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
    if !wants_json || informational {
        err.exit();
    }

    let rendered = err.render().to_string();
    let message = rendered
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("invalid arguments")
        .trim_start_matches("error: ")
        .to_string();
    devcli_core::output::print_json_error(&message);
    std::process::exit(1);
}

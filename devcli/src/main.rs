use clap::Parser;
use devcli::{cli, run, Cli};
use devcli_core::logging::init_tracing;

#[tokio::main]
async fn main() {
    // Rust ignores SIGPIPE, so printing into a closed pipe (`devcli config
    // show api | head -1`) panicked with "failed printing to stdout". Die
    // quietly instead, like other command-line tools.
    #[cfg(unix)]
    // SAFETY: restoring a signal's default disposition at start-up, before any
    // thread or handler depends on it.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

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
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    );
    if !wants_json || informational {
        err.exit();
    }

    // A missing subcommand (`devcli --json config`) is an error like any
    // other: it used to print clap's help on stdout and exit 2 under --json.
    if err.kind() == ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand {
        let words: Vec<String> = std::env::args()
            .skip(1)
            .filter(|a| !a.starts_with('-'))
            .collect();
        let command = std::iter::once("devcli".to_string())
            .chain(words)
            .collect::<Vec<_>>()
            .join(" ");
        devcli_core::output::print_json_error(&format!(
            "'{command}' requires a subcommand; run '{command} --help' to list them"
        ));
        std::process::exit(1);
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

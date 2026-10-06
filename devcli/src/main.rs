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
    let cli = Cli::parse();
    // `--json` implies machine-readable stdout; keep stderr free of log noise too.
    let enable_console = cli::console_tracing_enabled(&cli.command) && !cli.json;
    let _ = init_tracing(enable_console);
    run(cli).await
}

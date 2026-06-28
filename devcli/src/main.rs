use clap::Parser;
use devcli::{cli, run, Cli};
use devcli_core::logging::init_tracing;

#[tokio::main]
async fn main() {
    if let Err(e) = run_main().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run_main() -> devcli_core::Result<()> {
    let cli = Cli::parse();
    let enable_console = cli::console_tracing_enabled(&cli.command);
    let _ = init_tracing(enable_console);
    run(cli).await
}

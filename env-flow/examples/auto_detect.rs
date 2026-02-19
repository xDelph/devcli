//! Auto-detection: stage and runtime context inferred from process environment.
//!
//! Set APP_ENV=dev (or NODE_ENV, RUST_ENV, …) before running.
//! In Docker the runtime context is detected automatically via /.dockerenv.
use env_flow::EnvFlow;

fn main() -> env_flow::Result<()> {
    let vars = EnvFlow::from_dir(".").auto_detect().load()?;

    println!("Loaded {} variables (auto-detected context)", vars.len());
    for (key, value) in vars.iter() {
        println!("  {key} = {value}");
    }

    Ok(())
}

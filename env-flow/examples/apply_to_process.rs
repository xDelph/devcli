//! Apply loaded env vars to the process environment (std::env::set_var).
use env_flow::{EnvFlow, RuntimeContext, Stage};

fn main() -> env_flow::Result<()> {
    let vars = EnvFlow::from_dir(".")
        .stage(Stage::Dev)
        .context(RuntimeContext::Local)
        .load()?;

    // Verify required keys are present before applying
    vars.require(&["APP_NAME"])?;

    // Push all vars into std::env so child processes inherit them
    vars.apply();

    println!(
        "Applied {} env vars to process environment.",
        vars.len()
    );

    // Read back via std::env to confirm
    if let Ok(val) = std::env::var("APP_NAME") {
        println!("APP_NAME = {val}");
    }

    Ok(())
}

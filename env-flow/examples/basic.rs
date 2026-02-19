//! Basic usage: load env files from the current directory with an explicit stage.
use env_flow::{EnvFlow, RuntimeContext, Stage};

fn main() -> env_flow::Result<()> {
    let vars = EnvFlow::from_dir(".")
        .stage(Stage::Dev)
        .context(RuntimeContext::Local)
        .load()?;

    println!("Loaded {} variables", vars.len());
    for (key, value) in vars.iter() {
        println!("  {key} = {value}");
    }

    Ok(())
}

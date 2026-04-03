mod cli;
mod config;
mod discovery;
mod error;
mod runner;

pub use cli::{Action, Cli};
pub use error::Error;

use clap::Parser;
use std::env;
use std::path::Path;

pub fn run_from_args() -> Result<i32, Error> {
    let cli = Cli::parse();
    let cwd = env::current_dir()?;
    run_action(cli.action, &cwd)
}

pub fn run_action(action: Action, start_dir: &Path) -> Result<i32, Error> {
    let config_path = discovery::discover_config(start_dir)?;
    let config = config::ProjectConfig::load(&config_path)?;
    let status = runner::execute(action, &config)?;
    Ok(status.code().unwrap_or(1))
}

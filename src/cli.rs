use clap::{Parser, Subcommand};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
pub enum Action {
    Build,
    Test,
    Run,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Action::Build => "build",
            Action::Test => "test",
            Action::Run => "run",
        })
    }
}

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Build, test, and run projects from hidden config"
)]
pub struct Cli {
    #[command(subcommand)]
    pub action: Action,
}

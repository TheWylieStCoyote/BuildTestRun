use clap::{Args, Parser, Subcommand};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Action {
    Build(CommandArgs),
    Test(CommandArgs),
    Run(CommandArgs),
    Fmt(CommandArgs),
    Clean(CommandArgs),
    Ci(CommandArgs),
    Exec(ExecArgs),
    Validate,
    Init(InitArgs),
    List,
    Which,
    Doctor,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Build(_) => f.write_str("build"),
            Action::Test(_) => f.write_str("test"),
            Action::Run(_) => f.write_str("run"),
            Action::Fmt(_) => f.write_str("fmt"),
            Action::Clean(_) => f.write_str("clean"),
            Action::Ci(_) => f.write_str("ci"),
            Action::Exec(ExecArgs { name, .. }) => f.write_str(name),
            Action::Validate => f.write_str("validate"),
            Action::Init(_) => f.write_str("init"),
            Action::List => f.write_str("list"),
            Action::Which => f.write_str("which"),
            Action::Doctor => f.write_str("doctor"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct CommandArgs {
    #[arg(value_name = "ARGS", last = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ExecArgs {
    pub name: String,

    #[arg(value_name = "ARGS", last = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct InitArgs {
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Build, test, and run projects from hidden config"
)]
pub struct Cli {
    #[arg(long)]
    pub dry_run: bool,

    #[arg(long)]
    pub json: bool,

    #[command(subcommand)]
    pub action: Action,
}

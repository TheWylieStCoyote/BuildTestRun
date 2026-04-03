use clap::{Args, Parser, Subcommand, ValueEnum};
use std::{fmt, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Action {
    Build(CommandArgs),
    Test(CommandArgs),
    Run(CommandArgs),
    Fmt(CommandArgs),
    Clean(CommandArgs),
    Ci(CommandArgs),
    Exec(ExecArgs),
    Parallel(ParallelArgs),
    Validate(ValidateArgs),
    Init(InitArgs),
    Workspace(WorkspaceArgs),
    Package(PackageArgs),
    Completions(CompletionsArgs),
    Manpage,
    List(ListArgs),
    Which,
    Doctor(DoctorArgs),
    Show(ShowArgs),
    Explain(ShowArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum InitTemplate {
    Rust,
    Node,
    Pnpm,
    Yarn,
    Python,
    Poetry,
    Uv,
    Go,
    CargoWorkspace,
    Cmake,
    CmakeNinja,
    Generic,
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
            Action::Parallel(_) => f.write_str("parallel"),
            Action::Validate(_) => f.write_str("validate"),
            Action::Init(_) => f.write_str("init"),
            Action::Workspace(_) => f.write_str("workspace"),
            Action::Package(_) => f.write_str("package"),
            Action::Completions(_) => f.write_str("completions"),
            Action::Manpage => f.write_str("manpage"),
            Action::List(_) => f.write_str("list"),
            Action::Which => f.write_str("which"),
            Action::Doctor(_) => f.write_str("doctor"),
            Action::Show(_) => f.write_str("show"),
            Action::Explain(_) => f.write_str("explain"),
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
pub struct ParallelArgs {
    #[arg(required = true)]
    pub names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct InitArgs {
    #[arg(long)]
    pub force: bool,

    #[arg(long, value_enum, default_value_t = InitTemplate::Rust)]
    pub template: InitTemplate,

    #[arg(long)]
    pub interactive: bool,

    #[arg(long, value_name = "PATH")]
    pub template_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct WorkspaceArgs {
    #[arg(long)]
    pub list: bool,

    pub name: Option<String>,

    #[arg(value_name = "ARGS", last = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct PackageArgs {
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct CompletionsArgs {
    pub shell: CompletionShell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    PowerShell,
    Zsh,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ValidateArgs {
    #[arg(long)]
    pub strict: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ListArgs {
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct DoctorArgs {
    #[arg(long)]
    pub strict: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ShowArgs {
    pub name: String,

    #[arg(value_name = "ARGS", last = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
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
    pub safe: bool,

    #[arg(long)]
    pub json: bool,

    #[arg(long, value_name = "PATH")]
    pub workspace: Option<PathBuf>,

    #[command(subcommand)]
    pub action: Action,
}

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::{fmt, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Action {
    Build(CommandArgs),
    Test(CommandArgs),
    Run(CommandArgs),
    Dev(CommandArgs),
    Fmt(CommandArgs),
    Clean(CommandArgs),
    Ci(CommandArgs),
    Exec(ExecArgs),
    Parallel(ParallelArgs),
    Validate(ValidateArgs),
    Init(InitArgs),
    Templates(TemplatesArgs),
    Workspace(WorkspaceArgs),
    Watch(WatchArgs),
    Package(PackageArgs),
    Release(ReleaseArgs),
    Completions(CompletionsArgs),
    Schema,
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
    Bun,
    Deno,
    Nextjs,
    Vite,
    Turbo,
    Nx,
    Python,
    Django,
    Fastapi,
    Flask,
    Poetry,
    Hatch,
    Pixi,
    Uv,
    Go,
    CargoWorkspace,
    JavaGradle,
    JavaMaven,
    KotlinGradle,
    Dotnet,
    PhpComposer,
    RubyBundler,
    Rails,
    Laravel,
    Terraform,
    Helm,
    DockerCompose,
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
            Action::Dev(_) => f.write_str("dev"),
            Action::Fmt(_) => f.write_str("fmt"),
            Action::Clean(_) => f.write_str("clean"),
            Action::Ci(_) => f.write_str("ci"),
            Action::Exec(ExecArgs { name, .. }) => f.write_str(name),
            Action::Parallel(_) => f.write_str("parallel"),
            Action::Validate(_) => f.write_str("validate"),
            Action::Init(_) => f.write_str("init"),
            Action::Templates(_) => f.write_str("templates"),
            Action::Workspace(_) => f.write_str("workspace"),
            Action::Watch(_) => f.write_str("watch"),
            Action::Package(_) => f.write_str("package"),
            Action::Release(_) => f.write_str("release"),
            Action::Completions(_) => f.write_str("completions"),
            Action::Schema => f.write_str("schema"),
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

    #[arg(long = "import")]
    pub r#import: bool,

    #[arg(long, value_enum, default_value_t = InitTemplate::Rust)]
    pub template: InitTemplate,

    #[arg(long)]
    pub interactive: bool,

    #[arg(long)]
    pub detect: bool,

    #[arg(long)]
    pub print: bool,

    #[arg(long)]
    pub list_templates: bool,

    #[arg(long, value_name = "PATH")]
    pub template_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct TemplatesArgs {
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct WorkspaceArgs {
    #[arg(long)]
    pub list: bool,

    #[arg(long)]
    pub changed_only: bool,

    #[arg(long, value_name = "N")]
    pub jobs: Option<usize>,

    #[arg(long)]
    pub fail_fast: bool,

    #[arg(long)]
    pub keep_going: bool,

    #[arg(long, value_enum, default_value_t = WorkspaceOrder::Path)]
    pub order: WorkspaceOrder,

    #[arg(long, value_name = "REF")]
    pub since: Option<String>,

    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,

    #[arg(long = "tag", value_name = "TAG")]
    pub tags: Vec<String>,

    pub command: Option<String>,

    #[arg(value_name = "ARGS", last = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum WatchAction {
    Build(CommandArgs),
    Test(CommandArgs),
    Run(CommandArgs),
    Dev(CommandArgs),
    Fmt(CommandArgs),
    Clean(CommandArgs),
    Ci(CommandArgs),
    Workspace(WorkspaceArgs),
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct WatchArgs {
    #[command(subcommand)]
    pub action: WatchAction,

    #[arg(long)]
    pub once: bool,

    #[arg(long, value_name = "MILLIS", default_value_t = 1000)]
    pub poll_interval: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum WorkspaceOrder {
    Path,
    Name,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct PackageArgs {
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ReleaseArgs {
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

    #[arg(long)]
    pub fix: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ShowArgs {
    #[arg(long)]
    pub source: bool,

    #[arg(long)]
    pub tree: bool,

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

    #[arg(long = "json-events")]
    pub json_events: bool,

    #[arg(long, value_name = "DIR")]
    pub log_dir: Option<PathBuf>,

    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,

    #[arg(long, value_name = "PATH")]
    pub workspace: Option<PathBuf>,

    #[command(subcommand)]
    pub action: Action,
}

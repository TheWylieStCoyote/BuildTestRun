mod cli;
mod config;
mod discovery;
mod error;
mod runner;

pub use cli::{Action, Cli};
pub use error::Error;

use clap::{CommandFactory, Parser};
use serde_json::{Map, Value, json};
use std::{
    env, fs,
    io::{Seek, Write},
    path::{Path, PathBuf},
    thread,
};

pub fn run_from_args() -> Result<i32, Error> {
    let cli = Cli::parse();
    let cwd = env::current_dir()?;
    let start_dir = cli.workspace.unwrap_or(cwd);

    match cli.action {
        Action::Validate(args) => {
            validate_action(&start_dir, args.strict, cli.json, cli.profile.as_deref())
        }
        Action::Init(args) => init_action(
            &start_dir,
            args.force,
            args.template,
            args.interactive,
            args.list_templates,
            args.template_file,
            cli.json,
        ),
        Action::Templates(args) => templates_action(cli.json, args.verbose),
        Action::Workspace(args) => workspace_action(
            &start_dir,
            args.list,
            WorkspaceSelection {
                command_name: args.command,
                filter_name: args.name,
            },
            args.args,
            cli.json,
            cli.safe,
            cli.profile.as_deref(),
        ),
        Action::Package(args) => package_action(&start_dir, args.output, cli.json),
        Action::Release(args) => {
            release_action(&start_dir, args.output, cli.json, cli.profile.as_deref())
        }
        Action::Completions(args) => completions_action(args.shell),
        Action::Manpage => manpage_action(),
        Action::List(args) => {
            list_action(&start_dir, cli.json, args.verbose, cli.profile.as_deref())
        }
        Action::Which => which_action(&start_dir, cli.json, cli.profile.as_deref()),
        Action::Doctor(args) => {
            doctor_action(&start_dir, args.strict, cli.json, cli.profile.as_deref())
        }
        Action::Show(args) => show_action(
            &start_dir,
            args.name,
            args.args,
            cli.json,
            cli.profile.as_deref(),
        ),
        Action::Explain(args) => explain_action(
            &start_dir,
            args.name,
            args.args,
            cli.json,
            cli.profile.as_deref(),
        ),
        Action::Parallel(args) => parallel_action(
            &start_dir,
            args.names,
            cli.json,
            cli.dry_run,
            cli.safe,
            cli.profile.as_deref(),
        ),
        action => {
            if cli.dry_run {
                dry_run_action(
                    action,
                    &start_dir,
                    cli.json,
                    cli.safe,
                    cli.profile.as_deref(),
                )
            } else {
                run_action(action, &start_dir, cli.safe, cli.profile.as_deref())
            }
        }
    }
}

pub fn run_action(
    action: Action,
    start_dir: &Path,
    safe: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let (_, config) = load_project(start_dir, profile)?;
    let status = runner::execute(action, &config, safe)?;
    Ok(status.code().unwrap_or(1))
}

pub(crate) fn workspace_action(
    start_dir: &Path,
    list: bool,
    selection: WorkspaceSelection,
    args: Vec<String>,
    json_output: bool,
    safe: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let projects = discovery::discover_project_paths(start_dir)?;
    let projects =
        collect_workspace_projects(&projects, profile, selection.filter_name.as_deref())?;

    if list {
        let entries: Vec<_> = projects
            .iter()
            .map(|(path, config)| {
                Ok(json!({
                    "config": path,
                    "root": config.root,
                    "name": config.name,
                }))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        if json_output {
            print_stable_json(json!({"projects": entries}));
        } else {
            for entry in entries {
                if let Value::Object(map) = entry {
                    if let Some(config) = map.get("config").and_then(Value::as_str) {
                        println!("config: {config}");
                    }
                    if let Some(root) = map.get("root").and_then(Value::as_str) {
                        println!("root: {root}");
                    }
                    if let Some(name) = map.get("name").and_then(Value::as_str) {
                        println!("name: {name}");
                    }
                }
            }
        }

        return Ok(0);
    }

    let Some(command_name) = selection.command_name else {
        return Err(Error::Execution(
            "workspace requires a command name".to_string(),
        ));
    };

    let mut exit_code = 0;
    for (_, config) in projects {
        if !json_output {
            println!("[mbr] workspace: {}", config.root.display());
        }
        match runner::execute(
            Action::Exec(cli::ExecArgs {
                name: command_name.clone(),
                args: args.clone(),
            }),
            &config,
            safe,
        ) {
            Ok(status) => {
                if !status.success() {
                    exit_code = 1;
                }
            }
            Err(err) => return Err(err),
        }
    }

    Ok(exit_code)
}

struct WorkspaceSelection {
    command_name: Option<String>,
    filter_name: Option<String>,
}

fn collect_workspace_projects(
    projects: &[PathBuf],
    profile: Option<&str>,
    filter_name: Option<&str>,
) -> Result<Vec<(PathBuf, config::ProjectConfig)>, Error> {
    let mut entries = Vec::new();

    for path in projects {
        let config = config::ProjectConfig::load_inherited_with_profile(
            path.parent().unwrap_or(path),
            profile,
        )?;

        if filter_name.is_some_and(|expected| config.name.as_deref() != Some(expected)) {
            continue;
        }

        entries.push((path.clone(), config));
    }

    Ok(entries)
}

pub fn validate_action(
    start_dir: &Path,
    strict: bool,
    json_output: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir, profile)?;
    let warnings = if strict {
        validation_issues(&config)
    } else {
        Vec::new()
    };

    let exit_code = if strict && !warnings.is_empty() { 1 } else { 0 };

    if json_output {
        print_stable_json(json!({
            "status": if warnings.is_empty() { "ok" } else { "warn" },
            "config": config_path,
            "project": config.name,
            "warnings": warnings,
        }));
    } else if !warnings.is_empty() {
        for warning in &warnings {
            eprintln!("warning: {warning}");
        }
    } else if let Some(name) = config.name.as_deref() {
        eprintln!("[mbr] validated project: {name}");
    } else {
        eprintln!("[mbr] config valid");
    }

    Ok(exit_code)
}

pub fn init_action(
    start_dir: &Path,
    force: bool,
    template: cli::InitTemplate,
    interactive: bool,
    list_templates: bool,
    template_file: Option<PathBuf>,
    json_output: bool,
) -> Result<i32, Error> {
    if list_templates {
        return templates_action(json_output, false);
    }

    let path = start_dir.join(".mbr.toml");
    if path.exists() && !force {
        return Err(Error::ConfigExists { path });
    }

    let init_spec = if interactive {
        prompt_init_spec(template)?
    } else {
        InitSpec {
            project_name: "example".to_string(),
            project_root: ".".to_string(),
            template,
            safe_mode: false,
            optional_commands: Vec::new(),
        }
    };

    let rendered = render_init_template(&init_spec, template_file)?;

    fs::write(&path, rendered).map_err(|source| Error::ConfigWrite {
        path: path.clone(),
        source,
    })?;

    if json_output {
        print_stable_json(json!({"status": "ok", "path": path}));
    } else {
        eprintln!("[mbr] wrote {}", path.display());
        for warning in template_warnings(init_spec.template) {
            eprintln!("warning: {warning}");
        }
    }

    Ok(0)
}

struct InitSpec {
    project_name: String,
    project_root: String,
    template: cli::InitTemplate,
    safe_mode: bool,
    optional_commands: Vec<String>,
}

fn prompt_init_spec(default_template: cli::InitTemplate) -> Result<InitSpec, Error> {
    let project_name = prompt("Project name", "example")?;
    let project_root = prompt("Project root", ".")?;
    let template = prompt_template(default_template)?;
    let optional_commands = prompt_optional_commands(template)?;
    let safe_mode = prompt_yes_no("Enable safe structured-only mode", false)?;

    Ok(InitSpec {
        project_name,
        project_root,
        template,
        safe_mode,
        optional_commands,
    })
}

fn prompt(label: &str, default: &str) -> Result<String, Error> {
    use std::io::{stdin, stdout};

    print!("{label} [{default}]: ");
    stdout()
        .flush()
        .map_err(|source| Error::Execution(source.to_string()))?;

    let mut input = String::new();
    stdin()
        .read_line(&mut input)
        .map_err(|source| Error::Execution(source.to_string()))?;

    let value = input.trim();
    Ok(if value.is_empty() {
        default.to_string()
    } else {
        value.to_string()
    })
}

fn prompt_template(default_template: cli::InitTemplate) -> Result<cli::InitTemplate, Error> {
    use std::io::{stdin, stdout};

    println!("Choose a template:");
    for (idx, item) in template_variants().iter().enumerate() {
        println!("  {}. {}", idx + 1, init_template_name(*item));
    }

    print!("Template [{}]: ", init_template_name(default_template));
    stdout()
        .flush()
        .map_err(|source| Error::Execution(source.to_string()))?;

    let mut input = String::new();
    stdin()
        .read_line(&mut input)
        .map_err(|source| Error::Execution(source.to_string()))?;
    let value = input.trim();
    if value.is_empty() {
        return Ok(default_template);
    }

    if let Ok(index) = value.parse::<usize>()
        && let Some(template) = template_variants().get(index.saturating_sub(1))
    {
        return Ok(*template);
    }

    template_variants()
        .iter()
        .copied()
        .find(|template| init_template_name(*template).eq_ignore_ascii_case(value))
        .ok_or_else(|| Error::Execution(format!("unknown template selection: {value}")))
}

fn prompt_yes_no(label: &str, default: bool) -> Result<bool, Error> {
    use std::io::{stdin, stdout};

    let default_text = if default { "Y/n" } else { "y/N" };
    print!("{label} [{default_text}]: ");
    stdout()
        .flush()
        .map_err(|source| Error::Execution(source.to_string()))?;

    let mut input = String::new();
    stdin()
        .read_line(&mut input)
        .map_err(|source| Error::Execution(source.to_string()))?;

    let value = input.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Ok(default);
    }

    match value.as_str() {
        "y" | "yes" | "true" => Ok(true),
        "n" | "no" | "false" => Ok(false),
        _ => Err(Error::Execution(format!(
            "invalid yes/no response: {value}"
        ))),
    }
}

fn templates_action(json_output: bool, verbose: bool) -> Result<i32, Error> {
    let entries: Vec<_> = template_variants()
        .iter()
        .map(|template| {
            json!({
                "name": init_template_name(*template),
                "description": config::template_description(*template),
                "warning": config::template_spec(*template).warning,
            })
        })
        .collect();

    if json_output {
        print_stable_json(json!({
            "status": "ok",
            "count": entries.len(),
            "templates": entries,
        }));
    } else {
        for entry in entries {
            if let Value::Object(map) = entry {
                if let Some(name) = map.get("name").and_then(Value::as_str) {
                    print!("{name}");
                }
                if let Some(description) = map.get("description").and_then(Value::as_str) {
                    print!(" - {description}");
                }
                println!();
                if verbose && let Some(warning) = map.get("warning").and_then(Value::as_str) {
                    println!("  warning: {warning}");
                }
            }
        }
    }

    Ok(0)
}

fn template_variants() -> [cli::InitTemplate; 34] {
    [
        cli::InitTemplate::Rust,
        cli::InitTemplate::Node,
        cli::InitTemplate::Pnpm,
        cli::InitTemplate::Yarn,
        cli::InitTemplate::Bun,
        cli::InitTemplate::Deno,
        cli::InitTemplate::Nextjs,
        cli::InitTemplate::Vite,
        cli::InitTemplate::Turbo,
        cli::InitTemplate::Nx,
        cli::InitTemplate::Python,
        cli::InitTemplate::Django,
        cli::InitTemplate::Fastapi,
        cli::InitTemplate::Flask,
        cli::InitTemplate::Poetry,
        cli::InitTemplate::Hatch,
        cli::InitTemplate::Pixi,
        cli::InitTemplate::Uv,
        cli::InitTemplate::Go,
        cli::InitTemplate::CargoWorkspace,
        cli::InitTemplate::JavaGradle,
        cli::InitTemplate::JavaMaven,
        cli::InitTemplate::KotlinGradle,
        cli::InitTemplate::Dotnet,
        cli::InitTemplate::PhpComposer,
        cli::InitTemplate::RubyBundler,
        cli::InitTemplate::Rails,
        cli::InitTemplate::Laravel,
        cli::InitTemplate::Terraform,
        cli::InitTemplate::Helm,
        cli::InitTemplate::DockerCompose,
        cli::InitTemplate::Cmake,
        cli::InitTemplate::CmakeNinja,
        cli::InitTemplate::Generic,
    ]
}

struct OptionalPrompt {
    label: &'static str,
    command: &'static str,
}

const GENERIC_OPTIONAL_PROMPTS: [OptionalPrompt; 4] = [
    OptionalPrompt {
        label: "Include docs command",
        command: "docs",
    },
    OptionalPrompt {
        label: "Include dev command",
        command: "dev",
    },
    OptionalPrompt {
        label: "Include lint command",
        command: "lint",
    },
    OptionalPrompt {
        label: "Include typecheck command",
        command: "typecheck",
    },
];

const RUST_OPTIONAL_PROMPTS: [OptionalPrompt; 2] = [
    OptionalPrompt {
        label: "Include docs command",
        command: "docs",
    },
    OptionalPrompt {
        label: "Include lint command",
        command: "lint",
    },
];

const NODE_OPTIONAL_PROMPTS: [OptionalPrompt; 2] = [
    OptionalPrompt {
        label: "Include dev command",
        command: "dev",
    },
    OptionalPrompt {
        label: "Include typecheck command",
        command: "typecheck",
    },
];

const PYTHON_OPTIONAL_PROMPTS: [OptionalPrompt; 2] = [
    OptionalPrompt {
        label: "Include docs command",
        command: "docs",
    },
    OptionalPrompt {
        label: "Include lint command",
        command: "lint",
    },
];

fn prompt_optional_commands(template: cli::InitTemplate) -> Result<Vec<String>, Error> {
    let mut commands = Vec::new();
    for prompt in template_optional_prompts(template) {
        if prompt_yes_no(prompt.label, false)? {
            commands.push(prompt.command.to_string());
        }
    }

    Ok(commands)
}

fn template_optional_prompts(template: cli::InitTemplate) -> &'static [OptionalPrompt] {
    match template {
        cli::InitTemplate::Generic => &GENERIC_OPTIONAL_PROMPTS,
        cli::InitTemplate::Rust => &RUST_OPTIONAL_PROMPTS,
        cli::InitTemplate::Node => &NODE_OPTIONAL_PROMPTS,
        cli::InitTemplate::Python => &PYTHON_OPTIONAL_PROMPTS,
        _ => &[],
    }
}

fn render_init_template(spec: &InitSpec, template_file: Option<PathBuf>) -> Result<String, Error> {
    let contents = if let Some(path) = template_file {
        read_template_source(&path)?
    } else {
        config::starter_config_for(spec.template).to_string()
    };

    let rendered = rewrite_init_template(
        &contents,
        &spec.project_name,
        &spec.project_root,
        spec.template,
    );

    let rendered = if spec.optional_commands.is_empty() {
        rendered
    } else {
        append_generic_optional_commands(&rendered, &spec.optional_commands)
    };

    validate_rendered_init_template(&rendered)?;
    if spec.safe_mode {
        validate_safe_rendered_init_template(&rendered)?;
    }
    Ok(rendered)
}

fn read_template_source(path: &Path) -> Result<String, Error> {
    if path.is_dir() {
        for candidate in [".mbr.toml", "template.toml", "mbr.toml", "init.toml"] {
            let candidate_path = path.join(candidate);
            if candidate_path.is_file() {
                return fs::read_to_string(&candidate_path).map_err(|source| Error::TemplateRead {
                    path: candidate_path,
                    source,
                });
            }
        }

        return Err(Error::TemplateNotFound {
            path: path.to_path_buf(),
        });
    }

    fs::read_to_string(path).map_err(|source| Error::TemplateRead {
        path: path.to_path_buf(),
        source,
    })
}

fn validate_rendered_init_template(rendered: &str) -> Result<(), Error> {
    let parsed = toml::from_str::<config::ProjectFile>(rendered).map_err(|source| {
        Error::InitTemplateParse {
            source: Box::new(source),
        }
    })?;

    if parsed.commands.is_empty() {
        return Err(Error::MissingCommandGroup);
    }

    parsed
        .commands
        .resolve_inheritance()
        .map_err(|source| match source {
            Error::ConfigParse { path, source } => Error::ConfigParse { path, source },
            Error::UnknownCommandBase { name, base } => Error::UnknownCommandBase { name, base },
            Error::CommandInheritanceCycle { name } => Error::CommandInheritanceCycle { name },
            other => other,
        })?;
    Ok(())
}

fn validate_safe_rendered_init_template(rendered: &str) -> Result<(), Error> {
    let parsed = toml::from_str::<config::ProjectFile>(rendered).map_err(|source| {
        Error::InitTemplateParse {
            source: Box::new(source),
        }
    })?;

    let commands = parsed.commands.resolve_inheritance()?;

    for name in commands.names() {
        if let Some(command) = commands.get(&name)
            && command.is_shell()
        {
            return Err(Error::UnsafeInitTemplate { name });
        }
    }

    Ok(())
}

fn append_generic_optional_commands(rendered: &str, optional_commands: &[String]) -> String {
    let mut output = String::from(rendered);
    if !output.ends_with('\n') {
        output.push('\n');
    }

    for name in optional_commands {
        match name.as_str() {
            "docs" => output.push_str("docs = \"echo docs\"\n"),
            "dev" => output.push_str("dev = \"echo dev\"\n"),
            "lint" => output.push_str("lint = \"echo lint\"\n"),
            "typecheck" => output.push_str("typecheck = \"echo typecheck\"\n"),
            _ => {}
        }
    }

    output
}

fn rewrite_init_template(
    contents: &str,
    project_name: &str,
    project_root: &str,
    template: cli::InitTemplate,
) -> String {
    let replaced = contents
        .replace("{{project_name}}", project_name)
        .replace("{{project_root}}", project_root)
        .replace("{{template}}", init_template_name(template));

    let mut in_project = false;
    let mut replaced_name = false;
    let mut replaced_root = false;
    let mut lines = Vec::new();

    for line in replaced.lines() {
        let trimmed = line.trim();
        if trimmed == "[project]" {
            in_project = true;
            lines.push(line.to_string());
            continue;
        }

        if in_project && trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_project = false;
        }

        if in_project && !replaced_name && trimmed.starts_with("name = ") {
            lines.push(format!("name = \"{project_name}\""));
            replaced_name = true;
            continue;
        }

        if in_project && !replaced_root && trimmed.starts_with("root = ") {
            lines.push(format!("root = \"{project_root}\""));
            replaced_root = true;
            continue;
        }

        lines.push(line.to_string());
    }

    lines.join("\n")
}

fn init_template_name(template: cli::InitTemplate) -> &'static str {
    match template {
        cli::InitTemplate::Rust => "rust",
        cli::InitTemplate::Node => "node",
        cli::InitTemplate::Pnpm => "pnpm",
        cli::InitTemplate::Yarn => "yarn",
        cli::InitTemplate::Bun => "bun",
        cli::InitTemplate::Deno => "deno",
        cli::InitTemplate::Nextjs => "nextjs",
        cli::InitTemplate::Vite => "vite",
        cli::InitTemplate::Turbo => "turbo",
        cli::InitTemplate::Nx => "nx",
        cli::InitTemplate::Python => "python",
        cli::InitTemplate::Django => "django",
        cli::InitTemplate::Fastapi => "fastapi",
        cli::InitTemplate::Flask => "flask",
        cli::InitTemplate::Poetry => "poetry",
        cli::InitTemplate::Hatch => "hatch",
        cli::InitTemplate::Pixi => "pixi",
        cli::InitTemplate::Uv => "uv",
        cli::InitTemplate::Go => "go",
        cli::InitTemplate::CargoWorkspace => "cargo-workspace",
        cli::InitTemplate::JavaGradle => "java-gradle",
        cli::InitTemplate::JavaMaven => "java-maven",
        cli::InitTemplate::KotlinGradle => "kotlin-gradle",
        cli::InitTemplate::Dotnet => "dotnet",
        cli::InitTemplate::PhpComposer => "php-composer",
        cli::InitTemplate::RubyBundler => "ruby-bundler",
        cli::InitTemplate::Rails => "rails",
        cli::InitTemplate::Laravel => "laravel",
        cli::InitTemplate::Terraform => "terraform",
        cli::InitTemplate::Helm => "helm",
        cli::InitTemplate::DockerCompose => "docker-compose",
        cli::InitTemplate::Cmake => "cmake",
        cli::InitTemplate::CmakeNinja => "cmake-ninja",
        cli::InitTemplate::Generic => "generic",
    }
}

pub fn package_action(
    start_dir: &Path,
    output: Option<PathBuf>,
    json_output: bool,
) -> Result<i32, Error> {
    let (_, config) = load_project(start_dir, None)?;
    let archive_path = output.unwrap_or_else(|| default_package_path(&config));

    if cfg!(windows) {
        create_zip_package(&config.root, &archive_path)?;
    } else {
        create_tar_gz_package(&config.root, &archive_path)?;
    }

    if json_output {
        print_stable_json(json!({"output": archive_path, "root": config.root}));
    } else {
        println!("package: {}", archive_path.display());
    }

    Ok(0)
}

pub fn release_action(
    start_dir: &Path,
    output: Option<PathBuf>,
    json_output: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let (_, config) = load_project(start_dir, profile)?;

    for action in [
        Action::Build(cli::CommandArgs { args: vec![] }),
        Action::Test(cli::CommandArgs { args: vec![] }),
    ] {
        let status = runner::execute(action, &config, false)?;
        if !status.success() {
            return Ok(status.code().unwrap_or(1));
        }
    }

    package_action(start_dir, output, json_output)
}

pub fn completions_action(shell: cli::CompletionShell) -> Result<i32, Error> {
    let mut command = Cli::command();
    let shell = match shell {
        cli::CompletionShell::Bash => clap_complete::Shell::Bash,
        cli::CompletionShell::Elvish => clap_complete::Shell::Elvish,
        cli::CompletionShell::Fish => clap_complete::Shell::Fish,
        cli::CompletionShell::PowerShell => clap_complete::Shell::PowerShell,
        cli::CompletionShell::Zsh => clap_complete::Shell::Zsh,
    };
    clap_complete::generate(shell, &mut command, "mbr", &mut std::io::stdout());
    Ok(0)
}

pub fn manpage_action() -> Result<i32, Error> {
    let command = Cli::command();
    let man = clap_mangen::Man::new(command);
    man.render(&mut std::io::stdout())
        .map_err(|source| Error::Execution(source.to_string()))?;
    Ok(0)
}

pub fn list_action(
    start_dir: &Path,
    json_output: bool,
    verbose: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let (_, config) = load_project(start_dir, profile)?;
    let entries: Vec<_> = config
        .commands
        .names()
        .into_iter()
        .map(|name| {
            let description = config
                .commands
                .get(&name)
                .and_then(|command| command.description())
                .map(|description| description.to_string());
            (name, description)
        })
        .collect();

    if json_output {
        let commands: Vec<_> = entries
            .iter()
            .map(|(name, description)| json!({"name": name, "description": description}))
            .collect();
        print_stable_json(json!({"commands": commands}));
    } else {
        for (name, description) in entries {
            if verbose {
                if let Some(command) = config.commands.get(&name) {
                    println!("{name}");
                    if let Some(description) = description.as_deref() {
                        println!("  description: {description}");
                    }
                    println!("  command: {}", command.render(&[]));
                    if let Some(cwd) = command.cwd() {
                        println!("  cwd: {cwd}");
                    }
                    if let Some(timeout) = command.timeout() {
                        println!("  timeout: {timeout}s");
                    }
                    if command.is_pipeline() {
                        println!("  steps: {}", command.steps().join(", "));
                    }
                }
            } else {
                match description {
                    Some(description) => println!("{name} - {description}"),
                    None => println!("{name}"),
                }
            }
        }
    }

    Ok(0)
}

pub fn which_action(
    start_dir: &Path,
    json_output: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir, profile)?;

    if json_output {
        print_stable_json(json!({
            "config": config_path,
            "root": config.root,
        }));
    } else {
        println!("config: {}", config_path.display());
        println!("root: {}", config.root.display());
    }

    Ok(0)
}

pub fn doctor_action(
    start_dir: &Path,
    strict: bool,
    json_output: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir, profile)?;
    let warnings = validation_issues(&config);

    if json_output {
        print_stable_json(json!({
            "config": config_path,
            "root": config.root,
            "warnings": warnings,
            "status": if warnings.is_empty() { "ok" } else { "warn" },
        }));
    } else {
        println!("config: {}", config_path.display());
        println!("root: {}", config.root.display());
        if warnings.is_empty() {
            println!("status: ok");
        } else {
            for warning in &warnings {
                println!("warning: {warning}");
            }
        }
    }

    Ok(if strict && !warnings.is_empty() { 1 } else { 0 })
}

pub fn show_action(
    start_dir: &Path,
    name: String,
    args: Vec<String>,
    json_output: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    describe_action(start_dir, name, args, json_output, false, profile)
}

pub fn explain_action(
    start_dir: &Path,
    name: String,
    args: Vec<String>,
    json_output: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    describe_action(start_dir, name, args, json_output, true, profile)
}

fn describe_action(
    start_dir: &Path,
    name: String,
    args: Vec<String>,
    json_output: bool,
    explain: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir, profile)?;
    let command = config
        .commands
        .get(&name)
        .ok_or_else(|| Error::UnknownCommand { name: name.clone() })?;
    let rendered = command.render(&args);
    let cwd = command
        .cwd()
        .map(|path| resolve_workdir(&config.root, Some(path)))
        .unwrap_or_else(|| config.root.clone());

    if json_output {
        print_stable_json(json!({
            "config": config_path,
            "root": config.root,
            "name": name,
            "rendered": rendered,
            "cwd": cwd,
            "timeout": command.timeout(),
            "description": command.description(),
            "shell": command.is_shell(),
            "pipeline": command.is_pipeline(),
        }));
    } else {
        println!("name: {name}");
        println!("command: {rendered}");
        if explain {
            if command.is_pipeline() {
                println!("type: pipeline");
                println!("steps: {}", command.steps().join(" -> "));
            } else if command.is_shell() {
                println!("type: shell");
            } else {
                println!("type: program");
            }
        }
        println!("cwd: {}", cwd.display());
        if let Some(timeout) = command.timeout() {
            println!("timeout: {timeout}s");
        }
        if let Some(description) = command.description() {
            println!("description: {description}");
        }
    }

    Ok(0)
}

pub fn dry_run_action(
    action: Action,
    start_dir: &Path,
    json_output: bool,
    safe: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir, profile)?;
    trust_warning(&config);
    let (command_name, args) = action_command(&action);
    let command = config
        .commands
        .get(&command_name)
        .ok_or_else(|| unknown_command_error(&command_name))?;
    enforce_safe_command(&command_name, command, safe)?;
    let rendered = command.render(&args);

    if json_output {
        print_stable_json(json!({
            "config": config_path,
            "root": config.root,
            "command": command_name,
            "rendered": rendered,
        }));
    } else {
        println!("[mbr] dry-run: {rendered}");
    }

    Ok(0)
}

pub fn parallel_action(
    start_dir: &Path,
    names: Vec<String>,
    json_output: bool,
    dry_run: bool,
    safe: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let (_, config) = load_project(start_dir, profile)?;
    trust_warning(&config);

    if dry_run {
        let commands: Vec<_> = names
            .iter()
            .map(|name| {
                let command = config
                    .commands
                    .get(name)
                    .ok_or_else(|| Error::UnknownCommand { name: name.clone() })?;
                enforce_safe_command(name, command, safe)?;
                Ok(json!({
                    "name": name,
                    "rendered": command.render(&[]),
                    "timeout": command.timeout(),
                    "cwd": command.cwd(),
                }))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        if json_output {
            print_stable_json(json!({"parallel": commands}));
        } else {
            for name in names {
                let command = config
                    .commands
                    .get(&name)
                    .ok_or_else(|| Error::UnknownCommand { name: name.clone() })?;
                enforce_safe_command(&name, command, safe)?;
                println!("{name}: {}", command.render(&[]));
            }
        }

        return Ok(0);
    }

    let mut handles = Vec::new();
    for name in names.clone() {
        let config = config.clone();
        handles.push(thread::spawn(move || {
            runner::execute(
                Action::Exec(cli::ExecArgs {
                    name,
                    args: Vec::new(),
                }),
                &config,
                safe,
            )
        }));
    }

    let mut exit_code = 0;
    let mut errors = Vec::new();
    for handle in handles {
        match handle.join() {
            Ok(Ok(status)) => {
                if !status.success() {
                    exit_code = 1;
                }
            }
            Ok(Err(err)) => {
                errors.push(err.to_string());
                exit_code = 1;
            }
            Err(_) => {
                errors.push("parallel worker panicked".to_string());
                exit_code = 1;
            }
        }
    }

    if !errors.is_empty() {
        return Err(Error::Execution(errors.join("; ")));
    }

    if json_output {
        print_stable_json(json!({"parallel": names, "status": "ok"}));
    }

    Ok(exit_code)
}

fn program_on_path(program: &str) -> bool {
    let path = std::path::Path::new(program);
    if path.components().count() > 1 {
        return path.exists();
    }

    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };

    for dir in std::env::split_paths(&paths) {
        if cfg!(windows) {
            let exts = std::env::var_os("PATHEXT")
                .map(|value| {
                    value
                        .to_string_lossy()
                        .split(';')
                        .map(|ext| ext.trim().to_ascii_lowercase())
                        .filter(|ext| !ext.is_empty())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| {
                    vec![".exe".to_string(), ".cmd".to_string(), ".bat".to_string()]
                });

            for ext in exts {
                let candidate = dir.join(format!("{program}{ext}"));
                if candidate.is_file() {
                    return true;
                }
            }

            if dir.join(program).is_file() {
                return true;
            }
        } else if dir.join(program).is_file() {
            return true;
        }
    }

    false
}

fn load_project(
    start_dir: &Path,
    profile: Option<&str>,
) -> Result<(PathBuf, config::ProjectConfig), Error> {
    let config_path = discovery::discover_config(start_dir)?;
    let config = config::ProjectConfig::load_inherited_with_profile(start_dir, profile)?;
    Ok((config_path, config))
}

fn conventional_command_issues(config: &config::ProjectConfig) -> Vec<String> {
    let mut warnings = Vec::new();

    for builtin in ["build", "test", "run", "fmt", "clean", "ci"] {
        if config.commands.get(builtin).is_none() {
            warnings.push(format!("missing {builtin} command"));
        }
    }

    if config.commands.extra.is_empty() {
        warnings.push("no extra named commands defined".to_string());
    }

    warnings
}

fn validation_issues(config: &config::ProjectConfig) -> Vec<String> {
    let mut warnings = conventional_command_issues(config);

    if let Some(env_file) = config.env_file.as_deref()
        && !env_file_exists(&config.root, env_file)
    {
        warnings.push(format!("env file `{env_file}` was not found"));
    }

    if let Some(env_file) = config.profile_env_file.as_deref()
        && !env_file_exists(&config.root, env_file)
    {
        let profile = config
            .selected_profile
            .as_deref()
            .unwrap_or("selected profile");
        warnings.push(format!(
            "profile `{profile}` env file `{env_file}` was not found"
        ));
    }

    for name in config.commands.names() {
        if let Some(command) = config.commands.get(&name) {
            if let Some(program) = command.program() {
                if !program_on_path(program) {
                    warnings.push(format!(
                        "command `{name}` program `{program}` was not found on PATH"
                    ));
                }
            } else if command.is_shell() {
                warnings.push(format!(
                    "command `{name}` uses a shell string; PATH checks are skipped"
                ));
            }

            if let Some(message) = placeholder_run_warning(&name, command) {
                warnings.push(message);
            }
        }
    }

    warnings
}

fn env_file_exists(root: &Path, env_file: &str) -> bool {
    root.join(env_file).is_file()
}

fn placeholder_run_warning(name: &str, command: &config::CommandSpec) -> Option<String> {
    if name != "run" {
        return None;
    }

    let description = command
        .description()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if description.contains("placeholder") || description.contains("default target") {
        return Some(
            "command `run` appears to be a placeholder and should be customized".to_string(),
        );
    }

    if let Some(shell) = command.shell_command() {
        let normalized = shell.trim().to_ascii_lowercase();
        if normalized == "echo run" || normalized.contains("placeholder") {
            return Some(
                "command `run` appears to be a placeholder and should be customized".to_string(),
            );
        }
    }

    None
}

fn resolve_workdir(root: &Path, cwd: Option<&str>) -> PathBuf {
    match cwd {
        Some(value) => {
            let path = Path::new(value);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                root.join(path)
            }
        }
        None => root.to_path_buf(),
    }
}

fn default_package_path(config: &config::ProjectConfig) -> PathBuf {
    let stem = config
        .name
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string())
        .or_else(|| {
            config
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
        })
        .unwrap_or_else(|| "project".to_string());

    if cfg!(windows) {
        PathBuf::from(format!("{stem}.zip"))
    } else {
        PathBuf::from(format!("{stem}.tar.gz"))
    }
}

fn create_tar_gz_package(root: &Path, output: &Path) -> Result<(), Error> {
    let file = fs::File::create(output).map_err(|source| Error::Package(source.to_string()))?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let base_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");

    builder
        .append_dir_all(base_name, root)
        .map_err(|source| Error::Package(source.to_string()))?;

    let encoder = builder
        .into_inner()
        .map_err(|source| Error::Package(source.to_string()))?;
    encoder
        .finish()
        .map_err(|source| Error::Package(source.to_string()))?;
    Ok(())
}

fn create_zip_package(root: &Path, output: &Path) -> Result<(), Error> {
    let file = fs::File::create(output).map_err(|source| Error::Package(source.to_string()))?;
    let mut zip = zip::ZipWriter::new(file);
    add_zip_dir(
        &mut zip,
        root,
        root.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project"),
    )?;
    zip.finish()
        .map_err(|source| Error::Package(source.to_string()))?;
    Ok(())
}

fn add_zip_dir<W: Write + Seek>(
    zip: &mut zip::ZipWriter<W>,
    dir: &Path,
    prefix: &str,
) -> Result<(), Error> {
    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    for entry in fs::read_dir(dir).map_err(|source| Error::Package(source.to_string()))? {
        let entry = entry.map_err(|source| Error::Package(source.to_string()))?;
        let path = entry.path();
        let entry_name = entry.file_name().to_string_lossy().to_string();
        let name = format!("{prefix}/{entry_name}");

        if path.is_dir() {
            zip.add_directory(format!("{name}/"), options)
                .map_err(|source| Error::Package(source.to_string()))?;
            add_zip_dir(zip, &path, &name)?;
        } else if path.is_file() {
            zip.start_file(name, options)
                .map_err(|source| Error::Package(source.to_string()))?;
            let mut f =
                fs::File::open(&path).map_err(|source| Error::Package(source.to_string()))?;
            std::io::copy(&mut f, zip).map_err(|source| Error::Package(source.to_string()))?;
        }
    }

    Ok(())
}

fn template_warnings(template: cli::InitTemplate) -> Vec<&'static str> {
    vec![config::template_spec(template).warning]
}

fn action_command(action: &Action) -> (String, Vec<String>) {
    match action {
        Action::Build(args) => ("build".to_string(), args.args.clone()),
        Action::Test(args) => ("test".to_string(), args.args.clone()),
        Action::Run(args) => ("run".to_string(), args.args.clone()),
        Action::Dev(args) => ("dev".to_string(), args.args.clone()),
        Action::Fmt(args) => ("fmt".to_string(), args.args.clone()),
        Action::Clean(args) => ("clean".to_string(), args.args.clone()),
        Action::Ci(args) => ("ci".to_string(), args.args.clone()),
        Action::Exec(args) => (args.name.clone(), args.args.clone()),
        Action::Parallel(args) => ("parallel".to_string(), args.names.clone()),
        Action::Validate(_)
        | Action::Init(_)
        | Action::Templates(_)
        | Action::Workspace(_)
        | Action::Package(_)
        | Action::Release(_)
        | Action::Completions(_)
        | Action::Manpage
        | Action::List(_)
        | Action::Which
        | Action::Doctor(_)
        | Action::Show(_)
        | Action::Explain(_) => {
            unreachable!()
        }
    }
}

fn enforce_safe_command(
    name: &str,
    command: &config::CommandSpec,
    safe: bool,
) -> Result<(), Error> {
    if !safe {
        return Ok(());
    }

    if command.is_shell() {
        return Err(Error::UnsafeShellCommand {
            name: name.to_string(),
        });
    }

    Ok(())
}

fn trust_warning(config: &config::ProjectConfig) {
    if config.name.is_none() {
        eprintln!("[mbr] warning: project name is not set; command trust is lower");
    }
}

fn print_stable_json(value: Value) {
    println!("{}", stable_value(value));
}

fn stable_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut sorted = Map::new();
            for (key, value) in entries {
                sorted.insert(key, stable_value(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(stable_value).collect()),
        other => other,
    }
}

fn unknown_command_error(name: &str) -> Error {
    match name {
        "build" => Error::MissingCommand {
            action: Action::Build(cli::CommandArgs { args: vec![] }),
        },
        "test" => Error::MissingCommand {
            action: Action::Test(cli::CommandArgs { args: vec![] }),
        },
        "run" => Error::MissingCommand {
            action: Action::Run(cli::CommandArgs { args: vec![] }),
        },
        "fmt" => Error::MissingCommand {
            action: Action::Fmt(cli::CommandArgs { args: vec![] }),
        },
        "clean" => Error::MissingCommand {
            action: Action::Clean(cli::CommandArgs { args: vec![] }),
        },
        "ci" => Error::MissingCommand {
            action: Action::Ci(cli::CommandArgs { args: vec![] }),
        },
        other => Error::UnknownCommand {
            name: other.to_string(),
        },
    }
}

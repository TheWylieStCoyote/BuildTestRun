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
        Action::Validate(args) => validate_action(&start_dir, args.strict, cli.json),
        Action::Init(args) => init_action(&start_dir, args.force, args.template, cli.json),
        Action::Workspace(args) => workspace_action(
            &start_dir, args.list, args.name, args.args, cli.json, cli.safe,
        ),
        Action::Package(args) => package_action(&start_dir, args.output, cli.json),
        Action::Completions(args) => completions_action(args.shell),
        Action::Manpage => manpage_action(),
        Action::List(args) => list_action(&start_dir, cli.json, args.verbose),
        Action::Which => which_action(&start_dir, cli.json),
        Action::Doctor(args) => doctor_action(&start_dir, args.strict, cli.json),
        Action::Show(args) => show_action(&start_dir, args.name, args.args, cli.json),
        Action::Explain(args) => explain_action(&start_dir, args.name, args.args, cli.json),
        Action::Parallel(args) => {
            parallel_action(&start_dir, args.names, cli.json, cli.dry_run, cli.safe)
        }
        action => {
            if cli.dry_run {
                dry_run_action(action, &start_dir, cli.json, cli.safe)
            } else {
                run_action(action, &start_dir, cli.safe)
            }
        }
    }
}

pub fn run_action(action: Action, start_dir: &Path, safe: bool) -> Result<i32, Error> {
    let (_, config) = load_project(start_dir)?;
    let status = runner::execute(action, &config, safe)?;
    Ok(status.code().unwrap_or(1))
}

pub fn workspace_action(
    start_dir: &Path,
    list: bool,
    name: Option<String>,
    args: Vec<String>,
    json_output: bool,
    safe: bool,
) -> Result<i32, Error> {
    let projects = discovery::discover_project_paths(start_dir)?;

    if list {
        let entries: Vec<_> = projects
            .iter()
            .map(|path| {
                let config = config::ProjectConfig::load_inherited(path.parent().unwrap_or(path))?;
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

    let Some(command_name) = name else {
        return Err(Error::Execution(
            "workspace requires a command name".to_string(),
        ));
    };

    let mut exit_code = 0;
    for path in projects {
        let project_start = path.parent().unwrap_or(&path);
        let config = config::ProjectConfig::load_inherited(project_start)?;
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

pub fn validate_action(start_dir: &Path, strict: bool, json_output: bool) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir)?;
    let warnings = if strict {
        conventional_command_issues(&config)
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
    json_output: bool,
) -> Result<i32, Error> {
    let path = start_dir.join(".mbr.toml");
    if path.exists() && !force {
        return Err(Error::ConfigExists { path });
    }

    fs::write(&path, config::starter_config_for(template)).map_err(|source| {
        Error::ConfigWrite {
            path: path.clone(),
            source,
        }
    })?;

    if json_output {
        print_stable_json(json!({"status": "ok", "path": path}));
    } else {
        eprintln!("[mbr] wrote {}", path.display());
        for warning in template_warnings(template) {
            eprintln!("warning: {warning}");
        }
    }

    Ok(0)
}

pub fn package_action(
    start_dir: &Path,
    output: Option<PathBuf>,
    json_output: bool,
) -> Result<i32, Error> {
    let (_, config) = load_project(start_dir)?;
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

pub fn list_action(start_dir: &Path, json_output: bool, verbose: bool) -> Result<i32, Error> {
    let (_, config) = load_project(start_dir)?;
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

pub fn which_action(start_dir: &Path, json_output: bool) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir)?;

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

pub fn doctor_action(start_dir: &Path, strict: bool, json_output: bool) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir)?;
    let mut warnings = Vec::new();

    for builtin in ["build", "test", "run", "fmt", "clean", "ci"] {
        if config.commands.get(builtin).is_none() {
            warnings.push(format!("missing {builtin} command"));
        }
    }

    if config.commands.extra.is_empty() {
        warnings.push("no extra named commands defined".to_string());
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
        }
    }

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
) -> Result<i32, Error> {
    describe_action(start_dir, name, args, json_output, false)
}

pub fn explain_action(
    start_dir: &Path,
    name: String,
    args: Vec<String>,
    json_output: bool,
) -> Result<i32, Error> {
    describe_action(start_dir, name, args, json_output, true)
}

fn describe_action(
    start_dir: &Path,
    name: String,
    args: Vec<String>,
    json_output: bool,
    explain: bool,
) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir)?;
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
) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir)?;
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
) -> Result<i32, Error> {
    let (_, config) = load_project(start_dir)?;
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

fn load_project(start_dir: &Path) -> Result<(PathBuf, config::ProjectConfig), Error> {
    let config_path = discovery::discover_config(start_dir)?;
    let config = config::ProjectConfig::load_inherited(start_dir)?;
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
    match template {
        cli::InitTemplate::Rust => {
            vec!["Rust starter uses cargo shell-based ci; review before CI use"]
        }
        cli::InitTemplate::Node => {
            vec!["Node starter uses npm script conventions; ensure scripts exist"]
        }
        cli::InitTemplate::Pnpm => {
            vec!["pnpm starter assumes pnpm scripts exist in package.json"]
        }
        cli::InitTemplate::Yarn => {
            vec!["Yarn starter assumes yarn scripts exist in package.json"]
        }
        cli::InitTemplate::Python => vec!["Python starter assumes pytest and ruff are installed"],
        cli::InitTemplate::Poetry => {
            vec!["Poetry starter assumes poetry and project metadata are configured"]
        }
        cli::InitTemplate::Uv => vec!["uv starter assumes uv and Python tooling are installed"],
        cli::InitTemplate::Go => vec!["Go starter assumes gofmt and go tooling are installed"],
        cli::InitTemplate::CargoWorkspace => {
            vec!["Cargo workspace starter assumes a Rust workspace layout"]
        }
        cli::InitTemplate::Cmake => vec![
            "CMake starter uses a placeholder run target; replace it with your executable target",
        ],
        cli::InitTemplate::CmakeNinja => {
            vec!["CMake Ninja starter assumes Ninja and CMake are installed"]
        }
        cli::InitTemplate::Generic => {
            vec!["Generic starter is illustrative and should be customized"]
        }
    }
}

fn action_command(action: &Action) -> (String, Vec<String>) {
    match action {
        Action::Build(args) => ("build".to_string(), args.args.clone()),
        Action::Test(args) => ("test".to_string(), args.args.clone()),
        Action::Run(args) => ("run".to_string(), args.args.clone()),
        Action::Fmt(args) => ("fmt".to_string(), args.args.clone()),
        Action::Clean(args) => ("clean".to_string(), args.args.clone()),
        Action::Ci(args) => ("ci".to_string(), args.args.clone()),
        Action::Exec(args) => (args.name.clone(), args.args.clone()),
        Action::Parallel(args) => ("parallel".to_string(), args.names.clone()),
        Action::Validate(_)
        | Action::Init(_)
        | Action::Workspace(_)
        | Action::Package(_)
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

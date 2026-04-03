mod cli;
mod config;
mod discovery;
mod error;
mod runner;

pub use cli::{Action, Cli};
pub use error::Error;

use clap::Parser;
use serde_json::json;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub fn run_from_args() -> Result<i32, Error> {
    let cli = Cli::parse();
    let cwd = env::current_dir()?;

    match cli.action {
        Action::Validate => validate_action(&cwd, cli.json),
        Action::Init(args) => init_action(&cwd, args.force, args.template, cli.json),
        Action::List => list_action(&cwd, cli.json),
        Action::Which => which_action(&cwd, cli.json),
        Action::Doctor => doctor_action(&cwd, cli.json),
        action => {
            if cli.dry_run {
                dry_run_action(action, &cwd, cli.json)
            } else {
                run_action(action, &cwd)
            }
        }
    }
}

pub fn run_action(action: Action, start_dir: &Path) -> Result<i32, Error> {
    let config_path = discovery::discover_config(start_dir)?;
    let config = config::ProjectConfig::load(&config_path)?;
    let status = runner::execute(action, &config)?;
    Ok(status.code().unwrap_or(1))
}

pub fn validate_action(start_dir: &Path, json_output: bool) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir)?;
    if json_output {
        println!(
            "{}",
            json!({
                "status": "ok",
                "config": config_path,
                "project": config.name,
            })
        );
    } else if let Some(name) = config.name.as_deref() {
        eprintln!("[mbr] validated project: {name}");
    } else {
        eprintln!("[mbr] config valid");
    }
    Ok(0)
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
        println!("{}", json!({"status": "ok", "path": path}));
    } else {
        eprintln!("[mbr] wrote {}", path.display());
    }

    Ok(0)
}

pub fn list_action(start_dir: &Path, json_output: bool) -> Result<i32, Error> {
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
        println!("{}", json!({"commands": commands}));
    } else {
        for (name, description) in entries {
            match description {
                Some(description) => println!("{name} - {description}"),
                None => println!("{name}"),
            }
        }
    }

    Ok(0)
}

pub fn which_action(start_dir: &Path, json_output: bool) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir)?;

    if json_output {
        println!(
            "{}",
            json!({
                "config": config_path,
                "root": config.root,
            })
        );
    } else {
        println!("config: {}", config_path.display());
        println!("root: {}", config.root.display());
    }

    Ok(0)
}

pub fn doctor_action(start_dir: &Path, json_output: bool) -> Result<i32, Error> {
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
            match command {
                config::CommandSpec::Program { program, .. } => {
                    if !program_on_path(program) {
                        warnings.push(format!(
                            "command `{name}` program `{program}` was not found on PATH"
                        ));
                    }
                }
                config::CommandSpec::Shell(_) => {
                    warnings.push(format!(
                        "command `{name}` uses a shell string; PATH checks are skipped"
                    ));
                }
            }
        }
    }

    if json_output {
        println!(
            "{}",
            json!({
                "config": config_path,
                "root": config.root,
                "warnings": warnings,
            })
        );
    } else {
        println!("config: {}", config_path.display());
        println!("root: {}", config.root.display());
        if warnings.is_empty() {
            println!("status: ok");
        } else {
            for warning in warnings {
                println!("warning: {warning}");
            }
        }
    }

    Ok(0)
}

pub fn dry_run_action(action: Action, start_dir: &Path, json_output: bool) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir)?;
    let (command_name, args) = action_command(&action);
    let command = config
        .commands
        .get(&command_name)
        .ok_or_else(|| unknown_command_error(&command_name))?;
    let rendered = command.render(&args);

    if json_output {
        println!(
            "{}",
            json!({
                "config": config_path,
                "root": config.root,
                "command": command_name,
                "rendered": rendered,
            })
        );
    } else {
        println!("[mbr] dry-run: {rendered}");
    }

    Ok(0)
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
    let config = config::ProjectConfig::load(&config_path)?;
    Ok((config_path, config))
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
        Action::Validate | Action::Init(_) | Action::List | Action::Which | Action::Doctor => {
            unreachable!()
        }
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

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
        Action::Init(args) => init_action(&cwd, args.force, cli.json),
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

pub fn init_action(start_dir: &Path, force: bool, json_output: bool) -> Result<i32, Error> {
    let path = start_dir.join(".mbr.toml");
    if path.exists() && !force {
        return Err(Error::ConfigExists { path });
    }

    fs::write(&path, config::starter_config()).map_err(|source| Error::ConfigWrite {
        path: path.clone(),
        source,
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
    let mut names = config.commands.names();

    if json_output {
        println!("{}", json!({"commands": names}));
    } else {
        for name in names.drain(..) {
            println!("{name}");
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

    if config.commands.build.is_none() {
        warnings.push("missing build command".to_string());
    }
    if config.commands.test.is_none() {
        warnings.push("missing test command".to_string());
    }
    if config.commands.run.is_none() {
        warnings.push("missing run command".to_string());
    }
    if config.commands.extra.is_empty() {
        warnings.push("no extra named commands defined".to_string());
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
    let rendered = command.describe(&args);

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
        other => Error::UnknownCommand {
            name: other.to_string(),
        },
    }
}

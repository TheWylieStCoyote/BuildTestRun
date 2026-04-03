use crate::{
    cli::{Action, CommandArgs, ExecArgs},
    config::{CommandSpec, ProjectConfig},
    error::Error,
};
use std::{
    borrow::Cow,
    process::{Command, ExitStatus, Stdio},
};

pub fn execute(action: Action, config: &ProjectConfig) -> Result<ExitStatus, Error> {
    let (command_name, args, action_label) = match action {
        Action::Build(CommandArgs { args }) => ("build".to_string(), args, "build".to_string()),
        Action::Test(CommandArgs { args }) => ("test".to_string(), args, "test".to_string()),
        Action::Run(CommandArgs { args }) => ("run".to_string(), args, "run".to_string()),
        Action::Fmt(CommandArgs { args }) => ("fmt".to_string(), args, "fmt".to_string()),
        Action::Clean(CommandArgs { args }) => ("clean".to_string(), args, "clean".to_string()),
        Action::Ci(CommandArgs { args }) => ("ci".to_string(), args, "ci".to_string()),
        Action::Exec(ExecArgs { name, args }) => (name.clone(), args, name),
        Action::Validate | Action::Init(_) | Action::List | Action::Which | Action::Doctor => {
            unreachable!()
        }
    };

    let command = config
        .commands
        .get(&command_name)
        .ok_or_else(|| unknown_command_error(&command_name))?;

    if let Some(name) = config.name.as_deref() {
        eprintln!("[mbr] project: {name} | command: {action_label}");
    }

    let mut cmd = build_command(command, &args);
    cmd.current_dir(&config.root);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    for (key, value) in &config.env {
        cmd.env(key, value);
    }

    let status = cmd
        .status()
        .map_err(|source| Error::Execution(source.to_string()))?;
    Ok(status)
}

fn unknown_command_error(name: &str) -> Error {
    match name {
        "build" => Error::MissingCommand {
            action: Action::Build(CommandArgs { args: vec![] }),
        },
        "test" => Error::MissingCommand {
            action: Action::Test(CommandArgs { args: vec![] }),
        },
        "run" => Error::MissingCommand {
            action: Action::Run(CommandArgs { args: vec![] }),
        },
        "fmt" => Error::MissingCommand {
            action: Action::Fmt(CommandArgs { args: vec![] }),
        },
        "clean" => Error::MissingCommand {
            action: Action::Clean(CommandArgs { args: vec![] }),
        },
        "ci" => Error::MissingCommand {
            action: Action::Ci(CommandArgs { args: vec![] }),
        },
        other => Error::UnknownCommand {
            name: other.to_string(),
        },
    }
}

fn build_command(command: &CommandSpec, extra_args: &[String]) -> Command {
    match command {
        CommandSpec::Shell(base) => shell_command(base, extra_args),
        CommandSpec::Program { program, args, env } => {
            let mut cmd = Command::new(program);
            cmd.args(args).args(extra_args);
            for (key, value) in env {
                cmd.env(key, value);
            }
            cmd
        }
    }
}

fn shell_command(base: &str, extra_args: &[String]) -> Command {
    let command = if extra_args.is_empty() {
        Cow::Borrowed(base)
    } else {
        Cow::Owned(format!("{base} {}", render_args(extra_args)))
    };

    if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command.as_ref());
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command.as_ref());
        cmd
    }
}

fn render_args(args: &[String]) -> String {
    if cfg!(windows) {
        args.iter()
            .map(|arg| windows_quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        args.iter()
            .map(|arg| unix_quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn unix_quote(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }

    if arg.chars().all(|c| matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '-' | '.' | '/' | ':' | '@' | '%' | '+' | '=')) {
        return arg.to_string();
    }

    format!("'{}'", arg.replace('\'', "'\"'\"'"))
}

fn windows_quote(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }

    if arg
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '"' | '&' | '|' | '<' | '>' | '^'))
    {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

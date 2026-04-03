use crate::{
    cli::Action,
    config::{CommandsSection, ProjectConfig},
    error::Error,
};
use std::process::{Command, ExitStatus, Stdio};

pub fn execute(action: Action, config: &ProjectConfig) -> Result<ExitStatus, Error> {
    if let Some(name) = config.name.as_deref() {
        eprintln!("[mbr] project: {name} | action: {action}");
    }

    let command = command_for(&config.commands, action)?;

    let mut cmd = shell_command(config.shell.as_deref(), &command);
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

fn command_for(commands: &CommandsSection, action: Action) -> Result<String, Error> {
    match action {
        Action::Build => commands.build.clone(),
        Action::Test => commands.test.clone(),
        Action::Run => commands.run.clone(),
    }
    .ok_or(Error::MissingCommand { action })
}

fn shell_command(shell: Option<&str>, command: &str) -> Command {
    if let Some(shell) = shell {
        let mut cmd = Command::new(shell);
        cmd.arg(match cfg!(windows) {
            true => "/C",
            false => "-c",
        })
        .arg(command);
        return cmd;
    }

    if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    }
}

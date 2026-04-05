use crate::{
    cli::{Action, CommandArgs, ExecArgs},
    config::{CommandSpec, ProjectConfig},
    error::Error,
};
use std::{
    borrow::Cow,
    fs,
    fs::OpenOptions,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn execute(
    action: Action,
    config: &ProjectConfig,
    safe: bool,
    prefix: Option<&str>,
    json_output: bool,
    log_dir: Option<&PathBuf>,
) -> Result<ExitStatus, Error> {
    let (command_name, args, action_label) = match action {
        Action::Build(CommandArgs { args }) => ("build".to_string(), args, "build".to_string()),
        Action::Test(CommandArgs { args }) => ("test".to_string(), args, "test".to_string()),
        Action::Run(CommandArgs { args }) => ("run".to_string(), args, "run".to_string()),
        Action::Dev(CommandArgs { args }) => ("dev".to_string(), args, "dev".to_string()),
        Action::Fmt(CommandArgs { args }) => ("fmt".to_string(), args, "fmt".to_string()),
        Action::Clean(CommandArgs { args }) => ("clean".to_string(), args, "clean".to_string()),
        Action::Ci(CommandArgs { args }) => ("ci".to_string(), args, "ci".to_string()),
        Action::Exec(ExecArgs { name, args }) => (name.clone(), args, name),
        Action::Validate(_)
        | Action::Init(_)
        | Action::Templates(_)
        | Action::Workspace(_)
        | Action::Watch(_)
        | Action::Package(_)
        | Action::Release(_)
        | Action::Completions(_)
        | Action::Schema
        | Action::Manpage
        | Action::List(_)
        | Action::Which
        | Action::Doctor(_)
        | Action::Show(_)
        | Action::Explain(_)
        | Action::Parallel(_) => {
            unreachable!()
        }
    };
    if config.name.is_none() {
        eprintln!("[mbr] warning: project name is not set; command trust is lower");
    }

    run_named_command(
        &command_name,
        &args,
        &action_label,
        config,
        safe,
        prefix,
        json_output,
        log_dir,
    )
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
        "dev" => Error::MissingCommand {
            action: Action::Dev(CommandArgs { args: vec![] }),
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

#[allow(clippy::too_many_arguments)]
fn run_named_command(
    command_name: &str,
    args: &[String],
    action_label: &str,
    config: &ProjectConfig,
    safe: bool,
    prefix: Option<&str>,
    json_output: bool,
    log_dir: Option<&PathBuf>,
) -> Result<ExitStatus, Error> {
    let command = config
        .commands
        .get(command_name)
        .ok_or_else(|| unknown_command_error(command_name))?;

    if let Some(name) = config.name.as_deref() {
        eprintln!("[mbr] project: {name} | command: {action_label}");
    }

    if safe && command.is_shell() {
        return Err(Error::UnsafeShellCommand {
            name: command_name.to_string(),
        });
    }

    if command.is_pipeline() {
        if !args.is_empty() {
            return Err(Error::Execution(
                "pipeline commands do not accept extra args".to_string(),
            ));
        }

        let mut last_status = None;
        for step in command.steps() {
            last_status = Some(run_named_command(
                step,
                &[],
                step,
                config,
                safe,
                prefix,
                json_output,
                log_dir,
            )?);
        }

        return Ok(last_status.expect("pipeline commands must have at least one step"));
    }

    let retries = command.retries().unwrap_or(0);
    let mut attempt = 0;

    loop {
        let result = run_command_once(
            command,
            args,
            command_name,
            &config.root,
            &config.env,
            prefix,
            json_output,
            log_dir,
        );
        match result {
            Ok(status) if status.success() => return Ok(status),
            Ok(_) if attempt < retries => {
                attempt += 1;
                continue;
            }
            Ok(status) => return Ok(status),
            Err(_) if attempt < retries => {
                attempt += 1;
                continue;
            }
            Err(err) => return Err(err),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_command_once(
    command: &CommandSpec,
    extra_args: &[String],
    command_name: &str,
    root: &Path,
    project_env: &std::collections::HashMap<String, String>,
    prefix: Option<&str>,
    json_output: bool,
    log_dir: Option<&PathBuf>,
) -> Result<ExitStatus, Error> {
    let mut cmd = build_command(command, extra_args);
    cmd.current_dir(resolve_workdir(root, command.cwd()));
    cmd.stdin(Stdio::inherit());
    let capture_output = prefix.is_some() || log_dir.is_some();
    if let Some(log_dir) = log_dir {
        fs::create_dir_all(log_dir).map_err(|source| Error::Execution(source.to_string()))?;
    }
    if capture_output {
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
    } else if json_output {
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
    } else {
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
    }

    for (key, value) in project_env {
        cmd.env(key, value);
    }

    let mut child = cmd
        .spawn()
        .map_err(|source| Error::Execution(source.to_string()))?;

    if capture_output {
        let log_paths = log_dir.map(|dir| command_log_paths(dir, command_name));
        let emit_terminal = !json_output;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdout_thread = stdout.map(|stream| {
            spawn_output_reader(
                stream,
                prefix.map(|value| value.to_string()),
                emit_terminal,
                false,
                log_paths.as_ref().map(|(stdout, _)| stdout.clone()),
            )
        });
        let stderr_thread = stderr.map(|stream| {
            spawn_output_reader(
                stream,
                prefix.map(|value| value.to_string()),
                emit_terminal,
                true,
                log_paths.as_ref().map(|(_, stderr)| stderr.clone()),
            )
        });

        let status = match command.timeout() {
            Some(timeout_secs) => wait_with_timeout(&mut child, Duration::from_secs(timeout_secs)),
            None => child
                .wait()
                .map_err(|source| Error::Execution(source.to_string())),
        };

        if let Some(handle) = stdout_thread {
            let _ = handle.join();
        }
        if let Some(handle) = stderr_thread {
            let _ = handle.join();
        }

        return status;
    }

    match command.timeout() {
        Some(timeout_secs) => wait_with_timeout(&mut child, Duration::from_secs(timeout_secs)),
        None => child
            .wait()
            .map_err(|source| Error::Execution(source.to_string())),
    }
}

fn spawn_output_reader<R: std::io::Read + Send + 'static>(
    reader: R,
    prefix: Option<String>,
    emit_terminal: bool,
    is_err: bool,
    log_path: Option<PathBuf>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let reader = BufReader::new(reader);
        let mut log_file =
            log_path.and_then(|path| OpenOptions::new().create(true).append(true).open(path).ok());
        for line in reader.lines().map_while(Result::ok) {
            if let Some(file) = log_file.as_mut() {
                let _ = writeln!(file, "{line}");
            }
            if emit_terminal && is_err {
                let mut handle = std::io::stderr().lock();
                match prefix.as_deref() {
                    Some(prefix) => {
                        let _ = writeln!(handle, "[{prefix}] {line}");
                    }
                    None => {
                        let _ = writeln!(handle, "{line}");
                    }
                }
            } else if emit_terminal {
                let mut handle = std::io::stdout().lock();
                match prefix.as_deref() {
                    Some(prefix) => {
                        let _ = writeln!(handle, "[{prefix}] {line}");
                    }
                    None => {
                        let _ = writeln!(handle, "{line}");
                    }
                }
            }
        }
    })
}

fn command_log_paths(log_dir: &Path, command_name: &str) -> (PathBuf, PathBuf) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let safe_name = command_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let base = format!("{safe_name}-{stamp}");
    (
        log_dir.join(format!("{base}.stdout.log")),
        log_dir.join(format!("{base}.stderr.log")),
    )
}

fn build_command(command: &CommandSpec, extra_args: &[String]) -> Command {
    debug_assert!(!command.is_pipeline());
    match command.program() {
        None => shell_command(command.shell_command().unwrap_or_default(), extra_args),
        Some(program) => {
            let mut cmd = Command::new(program);
            cmd.args(command.args()).args(extra_args);
            for (key, value) in command.env() {
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

fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<ExitStatus, Error> {
    let start = Instant::now();

    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|source| Error::Execution(source.to_string()))?
        {
            return Ok(status);
        }

        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Execution(format!(
                "command timed out after {}s",
                timeout.as_secs()
            )));
        }

        thread::sleep(Duration::from_millis(100));
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

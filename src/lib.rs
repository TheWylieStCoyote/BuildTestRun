#![allow(clippy::result_large_err)]

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
    collections::HashSet,
    collections::VecDeque,
    env, fs,
    io::{Seek, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

fn json_envelope(command: &str, status: &str, fields: Vec<(&str, Value)>) -> Value {
    let mut object = Map::new();
    object.insert("status".to_string(), json!(status));
    object.insert("command".to_string(), json!(command));
    for (key, value) in fields {
        object.insert(key.to_string(), value);
    }
    Value::Object(object)
}

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
            InitOptions {
                r#import: args.r#import,
                interactive: args.interactive,
                detect: args.detect,
                print: args.print,
                list_templates: args.list_templates,
                template_file: args.template_file,
            },
            cli.json,
        ),
        Action::Templates(args) => templates_action(cli.json, args.verbose),
        Action::Workspace(args) => workspace_action(
            &start_dir,
            args.list,
            WorkspaceSelection {
                command_name: args.command,
                filter_name: args.name,
                tags: args.tags,
                changed_only: args.changed_only || args.since.is_some(),
                since: args.since,
                jobs: args.jobs,
                fail_fast: args.fail_fast,
                keep_going: args.keep_going,
                order: args.order,
            },
            args.args,
            cli.json,
            cli.json_events,
            cli.safe,
            cli.profile.as_deref(),
        ),
        Action::Watch(args) => {
            watch_action(&start_dir, args, cli.json, cli.safe, cli.profile.as_deref())
        }
        Action::Package(args) => package_action(&start_dir, args.output, cli.json),
        Action::Release(args) => release_action(
            &start_dir,
            args.output,
            cli.json,
            cli.json_events,
            cli.profile.as_deref(),
        ),
        Action::Completions(args) => completions_action(args.shell),
        Action::Manpage => manpage_action(),
        Action::List(args) => {
            list_action(&start_dir, cli.json, args.verbose, cli.profile.as_deref())
        }
        Action::Which => which_action(&start_dir, cli.json, cli.profile.as_deref()),
        Action::Doctor(args) => doctor_action(
            &start_dir,
            args.strict,
            args.fix,
            cli.json,
            cli.profile.as_deref(),
        ),
        Action::Show(args) => show_action(
            &start_dir,
            args.name,
            args.args,
            args.source,
            cli.json,
            cli.profile.as_deref(),
        ),
        Action::Explain(args) => explain_action(
            &start_dir,
            args.name,
            args.args,
            args.source,
            cli.json,
            cli.profile.as_deref(),
        ),
        Action::Parallel(args) => parallel_action(
            &start_dir,
            args.names,
            cli.json,
            cli.json_events,
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
    let started = Instant::now();
    let status = runner::execute(action.clone(), &config, safe, None, false)?;
    if !status.success() {
        print_failure_summary(
            None,
            Some(&action.to_string()),
            status.code(),
            started.elapsed(),
        );
    }
    print_command_summary(&action.to_string(), status.success(), 1, started.elapsed());
    Ok(status.code().unwrap_or(1))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn workspace_action(
    start_dir: &Path,
    list: bool,
    selection: WorkspaceSelection,
    args: Vec<String>,
    json_output: bool,
    json_events: bool,
    safe: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let started = Instant::now();
    let projects = discovery::discover_project_paths(start_dir)?;
    let projects = collect_workspace_projects(
        &projects,
        profile,
        selection.filter_name.as_deref(),
        &selection.tags,
    )?;
    let projects = if selection.changed_only {
        filter_changed_workspace_projects(start_dir, projects, selection.since.as_deref())?
    } else {
        projects
    };
    let projects = order_workspace_projects(projects, selection.order);

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
            print_stable_json(json_envelope(
                "workspace",
                "ok",
                vec![("projects", json!(entries))],
            ));
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

    if selection.fail_fast && selection.keep_going {
        return Err(Error::Execution(
            "workspace --fail-fast and --keep-going are mutually exclusive".to_string(),
        ));
    }

    let jobs = selection.jobs.unwrap_or(1);
    if jobs == 0 {
        return Err(Error::Execution(
            "workspace --jobs must be at least 1".to_string(),
        ));
    }

    if jobs > 1 {
        return execute_workspace_projects(
            &projects,
            WorkspaceRunOptions {
                command_name: &command_name,
                args: &args,
                safe,
                json_output,
                json_events,
                jobs,
                keep_going: selection.keep_going || !selection.fail_fast,
                fail_fast: selection.fail_fast,
                started,
            },
        );
    }

    let mut exit_code = 0;
    let mut executed = 0;
    let project_entries = if json_output {
        Some(
            projects
                .iter()
                .map(|(path, config)| {
                    json!({
                        "config": path,
                        "root": config.root,
                        "name": config.name,
                    })
                })
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };
    for (_, config) in projects {
        executed += 1;
        let started = Instant::now();
        emit_json_event(
            json_events,
            "workspace_command_start",
            vec![
                ("command", json!(command_name)),
                ("project", json!(config.name.clone())),
                ("root", json!(config.root.clone())),
            ],
        );
        if !json_output {
            let prefix = config
                .name
                .as_deref()
                .map(|name| format!("[{name}]"))
                .unwrap_or_else(|| format!("[{}]", config.root.display()));
            println!("{prefix} workspace: {}", config.root.display());
        }
        match runner::execute(
            Action::Exec(cli::ExecArgs {
                name: command_name.clone(),
                args: args.clone(),
            }),
            &config,
            safe,
            config.name.as_deref(),
            json_output,
        ) {
            Ok(status) => {
                emit_json_event(
                    json_events,
                    "workspace_command_finish",
                    vec![
                        ("command", json!(command_name)),
                        ("project", json!(config.name.clone())),
                        ("root", json!(config.root.clone())),
                        ("success", json!(status.success())),
                        ("exit_code", json!(status.code())),
                    ],
                );
                if !status.success() {
                    print_failure_summary(
                        config.name.as_deref(),
                        Some(command_name.as_str()),
                        status.code(),
                        started.elapsed(),
                    );
                    exit_code = 1;
                    if selection.fail_fast {
                        break;
                    }
                }
            }
            Err(err) => return Err(err),
        }
    }

    if let Some(projects) = project_entries {
        print_stable_json(json_envelope(
            "workspace",
            if exit_code == 0 { "ok" } else { "error" },
            vec![("projects", json!(projects))],
        ));
    }

    print_command_summary(
        &format!("workspace {command_name}"),
        exit_code == 0,
        executed,
        started.elapsed(),
    );

    Ok(exit_code)
}

struct WorkspaceSelection {
    command_name: Option<String>,
    filter_name: Option<String>,
    tags: Vec<String>,
    changed_only: bool,
    since: Option<String>,
    jobs: Option<usize>,
    fail_fast: bool,
    keep_going: bool,
    order: cli::WorkspaceOrder,
}

struct WorkspaceRunOptions<'a> {
    command_name: &'a str,
    args: &'a [String],
    safe: bool,
    json_output: bool,
    json_events: bool,
    jobs: usize,
    keep_going: bool,
    fail_fast: bool,
    started: Instant,
}

fn order_workspace_projects(
    mut projects: Vec<(PathBuf, config::ProjectConfig)>,
    order: cli::WorkspaceOrder,
) -> Vec<(PathBuf, config::ProjectConfig)> {
    match order {
        cli::WorkspaceOrder::Path => projects,
        cli::WorkspaceOrder::Name => {
            projects.sort_by(|left, right| {
                let left_name = left.1.name.as_deref().unwrap_or("").to_ascii_lowercase();
                let right_name = right.1.name.as_deref().unwrap_or("").to_ascii_lowercase();
                left_name
                    .cmp(&right_name)
                    .then_with(|| left.0.cmp(&right.0))
            });
            projects
        }
    }
}

fn execute_workspace_projects(
    projects: &[(PathBuf, config::ProjectConfig)],
    options: WorkspaceRunOptions<'_>,
) -> Result<i32, Error> {
    let WorkspaceRunOptions {
        command_name,
        args,
        safe,
        json_output,
        json_events,
        jobs,
        keep_going,
        fail_fast,
        started,
    } = options;
    let project_entries = if json_output {
        Some(
            projects
                .iter()
                .map(|(path, config)| {
                    json!({
                        "config": path,
                        "root": config.root,
                        "name": config.name,
                    })
                })
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };

    let mut exit_code = 0;
    let mut executed = 0usize;

    if jobs == 1 {
        for (_, config) in projects.iter() {
            executed += 1;
            let started = Instant::now();
            emit_json_event(
                json_events,
                "workspace_command_start",
                vec![
                    ("command", json!(command_name)),
                    ("project", json!(config.name.clone())),
                    ("root", json!(config.root.clone())),
                ],
            );
            if !json_output {
                let prefix = config
                    .name
                    .as_deref()
                    .map(|name| format!("[{name}]"))
                    .unwrap_or_else(|| format!("[{}]", config.root.display()));
                println!("{prefix} workspace: {}", config.root.display());
            }
            let status = runner::execute(
                Action::Exec(cli::ExecArgs {
                    name: command_name.to_string(),
                    args: args.to_vec(),
                }),
                config,
                safe,
                config.name.as_deref(),
                json_output,
            )?;
            emit_json_event(
                json_events,
                "workspace_command_finish",
                vec![
                    ("command", json!(command_name)),
                    ("project", json!(config.name.clone())),
                    ("root", json!(config.root.clone())),
                    ("success", json!(status.success())),
                    ("exit_code", json!(status.code())),
                ],
            );
            if !status.success() {
                print_failure_summary(
                    config.name.as_deref(),
                    Some(command_name),
                    status.code(),
                    started.elapsed(),
                );
                exit_code = 1;
                if fail_fast {
                    break;
                }
            }
        }
    } else {
        let queue = Arc::new(Mutex::new(VecDeque::from(projects.to_vec())));
        let failed = Arc::new(AtomicBool::new(false));
        let executed_count = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..jobs {
            let queue = Arc::clone(&queue);
            let failed = Arc::clone(&failed);
            let executed_count = Arc::clone(&executed_count);
            let command_name = command_name.to_string();
            let args = args.to_vec();
            let handle = thread::spawn(move || {
                let mut local_exit = 0;
                loop {
                    if fail_fast && failed.load(Ordering::SeqCst) {
                        break;
                    }

                    let next = {
                        let mut queue = queue.lock().expect("workspace queue");
                        queue.pop_front()
                    };

                    let Some((_, config)) = next else {
                        break;
                    };

                    executed_count.fetch_add(1, Ordering::SeqCst);
                    let started = Instant::now();
                    emit_json_event(
                        json_events,
                        "workspace_command_start",
                        vec![
                            ("command", json!(command_name.clone())),
                            ("project", json!(config.name.clone())),
                            ("root", json!(config.root.clone())),
                        ],
                    );
                    if !json_output {
                        let prefix = config
                            .name
                            .as_deref()
                            .map(|name| format!("[{name}]"))
                            .unwrap_or_else(|| format!("[{}]", config.root.display()));
                        println!("{prefix} workspace: {}", config.root.display());
                    }

                    match runner::execute(
                        Action::Exec(cli::ExecArgs {
                            name: command_name.clone(),
                            args: args.clone(),
                        }),
                        &config,
                        safe,
                        config.name.as_deref(),
                        json_output,
                    ) {
                        Ok(status) => {
                            emit_json_event(
                                json_events,
                                "workspace_command_finish",
                                vec![
                                    ("command", json!(command_name.clone())),
                                    ("project", json!(config.name.clone())),
                                    ("root", json!(config.root.clone())),
                                    ("success", json!(status.success())),
                                    ("exit_code", json!(status.code())),
                                ],
                            );
                            if !status.success() {
                                print_failure_summary(
                                    config.name.as_deref(),
                                    Some(command_name.as_str()),
                                    status.code(),
                                    started.elapsed(),
                                );
                                local_exit = 1;
                                failed.store(true, Ordering::SeqCst);
                            }
                        }
                        Err(err) => {
                            failed.store(true, Ordering::SeqCst);
                            local_exit = 1;
                            eprintln!(
                                "[mbr] failed: project={} | command={} | error={}",
                                config.name.as_deref().unwrap_or("(unnamed)"),
                                command_name,
                                err
                            );
                        }
                    }

                    if local_exit != 0 && !keep_going {
                        break;
                    }
                }

                local_exit
            });
            handles.push(handle);
        }

        for handle in handles {
            match handle.join() {
                Ok(status) => {
                    if status != 0 {
                        exit_code = 1;
                    }
                }
                Err(_) => {
                    exit_code = 1;
                }
            }
        }

        executed = executed_count.load(Ordering::SeqCst);
    }

    if let Some(projects) = project_entries {
        print_stable_json(json_envelope(
            "workspace",
            if exit_code == 0 { "ok" } else { "error" },
            vec![("projects", json!(projects))],
        ));
    }

    print_command_summary(
        &format!("workspace {command_name}"),
        exit_code == 0,
        executed,
        started.elapsed(),
    );

    Ok(exit_code)
}

pub(crate) fn watch_action(
    start_dir: &Path,
    args: cli::WatchArgs,
    json_output: bool,
    safe: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let interval = Duration::from_millis(args.poll_interval.max(1));
    let mut last_snapshot = snapshot_watch_tree(start_dir)?;

    loop {
        let exit_code = match &args.action {
            cli::WatchAction::Build(command_args) => run_watch_command(
                Action::Build(command_args.clone()),
                start_dir,
                json_output,
                safe,
                profile,
            )?,
            cli::WatchAction::Test(command_args) => run_watch_command(
                Action::Test(command_args.clone()),
                start_dir,
                json_output,
                safe,
                profile,
            )?,
            cli::WatchAction::Run(command_args) => run_watch_command(
                Action::Run(command_args.clone()),
                start_dir,
                json_output,
                safe,
                profile,
            )?,
            cli::WatchAction::Dev(command_args) => run_watch_command(
                Action::Dev(command_args.clone()),
                start_dir,
                json_output,
                safe,
                profile,
            )?,
            cli::WatchAction::Fmt(command_args) => run_watch_command(
                Action::Fmt(command_args.clone()),
                start_dir,
                json_output,
                safe,
                profile,
            )?,
            cli::WatchAction::Clean(command_args) => run_watch_command(
                Action::Clean(command_args.clone()),
                start_dir,
                json_output,
                safe,
                profile,
            )?,
            cli::WatchAction::Ci(command_args) => run_watch_command(
                Action::Ci(command_args.clone()),
                start_dir,
                json_output,
                safe,
                profile,
            )?,
            cli::WatchAction::Workspace(workspace_args) => workspace_action(
                start_dir,
                workspace_args.list,
                WorkspaceSelection {
                    command_name: workspace_args.command.clone(),
                    filter_name: workspace_args.name.clone(),
                    tags: workspace_args.tags.clone(),
                    changed_only: workspace_args.changed_only || workspace_args.since.is_some(),
                    since: workspace_args.since.clone(),
                    jobs: workspace_args.jobs,
                    fail_fast: workspace_args.fail_fast,
                    keep_going: workspace_args.keep_going,
                    order: workspace_args.order,
                },
                workspace_args.args.clone(),
                json_output,
                false,
                safe,
                profile,
            )?,
        };

        if args.once {
            return Ok(exit_code);
        }

        let current_snapshot = snapshot_watch_tree(start_dir)?;
        if current_snapshot != last_snapshot {
            last_snapshot = current_snapshot;
            continue;
        }

        thread::sleep(interval);
    }
}

fn run_watch_command(
    action: Action,
    start_dir: &Path,
    _json_output: bool,
    safe: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    run_action(action, start_dir, safe, profile)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WatchEntry {
    path: PathBuf,
    modified: Option<SystemTime>,
    len: u64,
}

fn snapshot_watch_tree(start_dir: &Path) -> Result<Vec<WatchEntry>, Error> {
    let mut entries = Vec::new();
    collect_watch_entries(start_dir, &mut entries)?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn collect_watch_entries(dir: &Path, entries: &mut Vec<WatchEntry>) -> Result<(), Error> {
    for entry in fs::read_dir(dir).map_err(|source| Error::Execution(source.to_string()))? {
        let entry = entry.map_err(|source| Error::Execution(source.to_string()))?;
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");

        if path.is_dir() {
            if should_skip_watch_dir(file_name) {
                continue;
            }
            collect_watch_entries(&path, entries)?;
        } else if path.is_file() {
            let metadata = entry
                .metadata()
                .map_err(|source| Error::Execution(source.to_string()))?;
            entries.push(WatchEntry {
                path,
                modified: metadata.modified().ok(),
                len: metadata.len(),
            });
        }
    }

    Ok(())
}

fn should_skip_watch_dir(name: &str) -> bool {
    matches!(name, ".git" | "target" | "node_modules" | "dist" | "build")
}

fn collect_workspace_projects(
    projects: &[PathBuf],
    profile: Option<&str>,
    filter_name: Option<&str>,
    filter_tags: &[String],
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

        if !filter_tags.is_empty()
            && !filter_tags
                .iter()
                .all(|tag| config.tags.iter().any(|existing| existing == tag))
        {
            continue;
        }

        entries.push((path.clone(), config));
    }

    Ok(entries)
}

fn filter_changed_workspace_projects(
    start_dir: &Path,
    projects: Vec<(PathBuf, config::ProjectConfig)>,
    since: Option<&str>,
) -> Result<Vec<(PathBuf, config::ProjectConfig)>, Error> {
    let changed_paths = git_changed_paths(start_dir, since)?;
    Ok(projects
        .into_iter()
        .filter(|(path, config)| {
            let project_root = config
                .root
                .canonicalize()
                .unwrap_or_else(|_| config.root.clone());
            let config_dir = path.parent().unwrap_or(path);
            let config_root = config_dir
                .canonicalize()
                .unwrap_or_else(|_| config_dir.to_path_buf());

            changed_paths.iter().any(|changed| {
                changed.starts_with(&project_root) || changed.starts_with(&config_root)
            })
        })
        .collect())
}

fn git_changed_paths(start_dir: &Path, since: Option<&str>) -> Result<Vec<PathBuf>, Error> {
    let repo_root = git_repo_root(start_dir)?;
    let mut changed = Vec::new();

    if let Some(since) = since {
        for args in [
            vec!["diff", "--name-only", since],
            vec!["diff", "--name-only", "--cached", since],
            vec!["ls-files", "--others", "--exclude-standard"],
        ] {
            let output = std::process::Command::new("git")
                .current_dir(start_dir)
                .args(args)
                .output()
                .map_err(|source| Error::Execution(source.to_string()))?;

            if !output.status.success() {
                return Err(Error::Execution("failed to query git changes".to_string()));
            }

            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let path = line.trim();
                if path.is_empty() {
                    continue;
                }
                changed.push(repo_root.join(path));
            }
        }
    } else {
        let output = std::process::Command::new("git")
            .current_dir(start_dir)
            .args(["status", "--porcelain=1", "--untracked-files=all"])
            .output()
            .map_err(|source| Error::Execution(source.to_string()))?;

        if !output.status.success() {
            return Err(Error::Execution("failed to query git changes".to_string()));
        }

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.len() < 4 {
                continue;
            }

            let path = line[3..].trim();
            if path.is_empty() {
                continue;
            }

            let path = path.split(" -> ").last().unwrap_or(path);
            changed.push(repo_root.join(path));
        }
    }

    changed.sort();
    changed.dedup();
    Ok(changed)
}

fn git_repo_root(start_dir: &Path) -> Result<PathBuf, Error> {
    let output = std::process::Command::new("git")
        .current_dir(start_dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|source| Error::Execution(source.to_string()))?;

    if !output.status.success() {
        return Err(Error::Execution(
            "failed to resolve git repository root".to_string(),
        ));
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return Err(Error::Execution(
            "failed to resolve git repository root".to_string(),
        ));
    }

    Ok(PathBuf::from(root))
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
        print_stable_json(json_envelope(
            "validate",
            if warnings.is_empty() { "ok" } else { "warn" },
            vec![
                ("config", json!(config_path)),
                ("project", json!(config.name)),
                ("warnings", json!(warnings)),
            ],
        ));
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

pub(crate) fn init_action(
    start_dir: &Path,
    force: bool,
    template: cli::InitTemplate,
    options: InitOptions,
    json_output: bool,
) -> Result<i32, Error> {
    if options.list_templates {
        return templates_action(json_output, false);
    }

    if options.r#import && options.template_file.is_some() {
        return Err(Error::Execution(
            "--import cannot be combined with --template-file".to_string(),
        ));
    }

    let template = if options.detect {
        detect_init_template(start_dir).unwrap_or(template)
    } else {
        template
    };

    let path = start_dir.join(".mbr.toml");
    if !options.print && path.exists() && !force {
        return Err(Error::ConfigExists { path });
    }

    let init_spec = if options.interactive {
        Some(prompt_init_spec(template)?)
    } else {
        None
    };

    let rendered = if options.r#import {
        match import_init_template(start_dir)? {
            Some(rendered) => rendered,
            None => {
                let init_spec = init_spec.unwrap_or_else(|| default_init_spec(template));
                render_init_template(&init_spec, None)?
            }
        }
    } else {
        let init_spec = init_spec.unwrap_or_else(|| default_init_spec(template));
        render_init_template(&init_spec, options.template_file)?
    };

    if options.print {
        if json_output {
            print_stable_json(json_envelope(
                "init",
                "ok",
                vec![("rendered", json!(rendered)), ("printed", json!(true))],
            ));
        } else {
            print!("{rendered}");
            if !rendered.ends_with('\n') {
                println!();
            }
        }
        return Ok(0);
    }

    fs::write(&path, rendered).map_err(|source| Error::ConfigWrite {
        path: path.clone(),
        source,
    })?;

    if json_output {
        print_stable_json(json_envelope("init", "ok", vec![("path", json!(path))]));
    } else {
        eprintln!("[mbr] wrote {}", path.display());
        if !options.r#import {
            for warning in template_warnings(template) {
                eprintln!("warning: {warning}");
            }
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

fn default_init_spec(template: cli::InitTemplate) -> InitSpec {
    InitSpec {
        project_name: "example".to_string(),
        project_root: ".".to_string(),
        template,
        safe_mode: false,
        optional_commands: Vec::new(),
    }
}

struct InitOptions {
    r#import: bool,
    interactive: bool,
    detect: bool,
    print: bool,
    list_templates: bool,
    template_file: Option<PathBuf>,
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

fn detect_init_template(start_dir: &Path) -> Option<cli::InitTemplate> {
    for dir in start_dir.ancestors() {
        if let Some(template) = detect_template_in_dir(dir) {
            return Some(template);
        }
    }

    None
}

fn detect_template_in_dir(dir: &Path) -> Option<cli::InitTemplate> {
    let cargo_toml = dir.join("Cargo.toml");
    if cargo_toml.is_file() {
        let contents = fs::read_to_string(&cargo_toml).ok()?;
        if contents.contains("[workspace]") {
            return Some(cli::InitTemplate::CargoWorkspace);
        }
        return Some(cli::InitTemplate::Rust);
    }

    if dir.join("package.json").is_file() {
        return Some(cli::InitTemplate::Node);
    }

    if dir.join("pyproject.toml").is_file() {
        return Some(cli::InitTemplate::Python);
    }

    if dir.join("CMakeLists.txt").is_file() {
        return Some(cli::InitTemplate::Cmake);
    }

    None
}

fn import_init_template(start_dir: &Path) -> Result<Option<String>, Error> {
    if let Some(rendered) = import_from_cargo(start_dir)? {
        return Ok(Some(rendered));
    }
    if let Some(rendered) = import_from_package_json(start_dir)? {
        return Ok(Some(rendered));
    }
    if let Some(rendered) = import_from_pyproject(start_dir)? {
        return Ok(Some(rendered));
    }
    if let Some(rendered) = import_from_makefile(start_dir)? {
        return Ok(Some(rendered));
    }
    if let Some(rendered) = import_from_justfile(start_dir)? {
        return Ok(Some(rendered));
    }

    Ok(None)
}

fn import_from_cargo(start_dir: &Path) -> Result<Option<String>, Error> {
    let path = start_dir.join("Cargo.toml");
    if !path.is_file() {
        return Ok(None);
    }

    let contents =
        fs::read_to_string(&path).map_err(|source| Error::Execution(source.to_string()))?;
    let parsed: toml::Value = toml::from_str(&contents)
        .map_err(|source| Error::Execution(format!("failed to parse Cargo.toml: {source}")))?;
    let project_name = parsed
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .map(|value| value.to_string())
        .unwrap_or_else(|| default_project_name(start_dir));
    let template = if parsed.get("workspace").is_some() {
        cli::InitTemplate::CargoWorkspace
    } else {
        cli::InitTemplate::Rust
    };

    render_init_template(
        &InitSpec {
            project_name,
            project_root: ".".to_string(),
            template,
            safe_mode: false,
            optional_commands: Vec::new(),
        },
        None,
    )
    .map(Some)
}

fn import_from_package_json(start_dir: &Path) -> Result<Option<String>, Error> {
    let path = start_dir.join("package.json");
    if !path.is_file() {
        return Ok(None);
    }

    let contents =
        fs::read_to_string(&path).map_err(|source| Error::Execution(source.to_string()))?;
    let parsed: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|source| Error::Execution(format!("failed to parse package.json: {source}")))?;
    let project_name = parsed
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(|value| value.to_string())
        .unwrap_or_else(|| default_project_name(start_dir));
    let template = package_manager_template(start_dir, &parsed);
    let scripts = parsed
        .get("scripts")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();

    if scripts.is_empty() {
        return render_init_template(
            &InitSpec {
                project_name,
                project_root: ".".to_string(),
                template,
                safe_mode: false,
                optional_commands: Vec::new(),
            },
            None,
        )
        .map(Some);
    }

    let manager = package_manager_name(start_dir, &parsed);
    let mut commands = Vec::new();
    let mut script_names = Vec::new();
    for (name, script) in scripts {
        if script.as_str().is_some() {
            let script_name = name.clone();
            script_names.push(script_name.clone());
            commands.push(ImportedCommand {
                name: script_name.clone(),
                program: manager.clone(),
                args: vec!["run".to_string(), script_name.clone()],
                description: Some(format!("Run `{script_name}` script")),
            });
        }
    }

    if script_names.iter().any(|name| name == "start")
        && !script_names.iter().any(|name| name == "run")
    {
        commands.push(ImportedCommand {
            name: "run".to_string(),
            program: manager.clone(),
            args: vec!["run".to_string(), "start".to_string()],
            description: Some("Start the app".to_string()),
        });
    } else if script_names.iter().any(|name| name == "dev")
        && !script_names.iter().any(|name| name == "run")
    {
        commands.push(ImportedCommand {
            name: "run".to_string(),
            program: manager.clone(),
            args: vec!["run".to_string(), "dev".to_string()],
            description: Some("Start the dev server".to_string()),
        });
    }

    Ok(Some(render_imported_config(project_name, commands)))
}

fn import_from_pyproject(start_dir: &Path) -> Result<Option<String>, Error> {
    let path = start_dir.join("pyproject.toml");
    if !path.is_file() {
        return Ok(None);
    }

    let contents =
        fs::read_to_string(&path).map_err(|source| Error::Execution(source.to_string()))?;
    let parsed: toml::Value = toml::from_str(&contents)
        .map_err(|source| Error::Execution(format!("failed to parse pyproject.toml: {source}")))?;
    let project_name = parsed
        .get("project")
        .and_then(|project| project.get("name"))
        .and_then(toml::Value::as_str)
        .or_else(|| {
            parsed
                .get("tool")
                .and_then(|tool| tool.get("poetry"))
                .and_then(|poetry| poetry.get("name"))
                .and_then(toml::Value::as_str)
        })
        .map(|value| value.to_string())
        .unwrap_or_else(|| default_project_name(start_dir));
    let template = if parsed
        .get("tool")
        .and_then(|tool| tool.get("poetry"))
        .is_some()
    {
        cli::InitTemplate::Poetry
    } else if parsed
        .get("tool")
        .and_then(|tool| tool.get("hatch"))
        .is_some()
    {
        cli::InitTemplate::Hatch
    } else if parsed
        .get("tool")
        .and_then(|tool| tool.get("pixi"))
        .is_some()
    {
        cli::InitTemplate::Pixi
    } else if parsed.get("tool").and_then(|tool| tool.get("uv")).is_some() {
        cli::InitTemplate::Uv
    } else {
        cli::InitTemplate::Python
    };

    render_init_template(
        &InitSpec {
            project_name,
            project_root: ".".to_string(),
            template,
            safe_mode: false,
            optional_commands: Vec::new(),
        },
        None,
    )
    .map(Some)
}

fn import_from_makefile(start_dir: &Path) -> Result<Option<String>, Error> {
    let path = find_project_file(start_dir, &["Makefile", "makefile", "GNUmakefile"]);
    let Some(path) = path else {
        return Ok(None);
    };

    let contents =
        fs::read_to_string(&path).map_err(|source| Error::Execution(source.to_string()))?;
    let targets = parse_make_targets(&contents);
    if targets.is_empty() {
        return Ok(Some(render_imported_config(
            default_project_name(start_dir),
            vec![ImportedCommand {
                name: "build".to_string(),
                program: "make".to_string(),
                args: vec!["build".to_string()],
                description: Some("Run the build target".to_string()),
            }],
        )));
    }

    let commands = targets
        .into_iter()
        .map(|name| ImportedCommand {
            program: "make".to_string(),
            args: vec![name.clone()],
            description: Some(format!("Run `make {name}`")),
            name,
        })
        .collect();

    Ok(Some(render_imported_config(
        default_project_name(start_dir),
        commands,
    )))
}

fn import_from_justfile(start_dir: &Path) -> Result<Option<String>, Error> {
    let path = find_project_file(start_dir, &["Justfile", "justfile"]);
    let Some(path) = path else {
        return Ok(None);
    };

    let contents =
        fs::read_to_string(&path).map_err(|source| Error::Execution(source.to_string()))?;
    let recipes = parse_just_recipes(&contents);
    if recipes.is_empty() {
        return Ok(Some(render_imported_config(
            default_project_name(start_dir),
            vec![ImportedCommand {
                name: "build".to_string(),
                program: "just".to_string(),
                args: vec!["build".to_string()],
                description: Some("Run the build recipe".to_string()),
            }],
        )));
    }

    let commands = recipes
        .into_iter()
        .map(|name| ImportedCommand {
            program: "just".to_string(),
            args: vec![name.clone()],
            description: Some(format!("Run `{name}`")),
            name,
        })
        .collect();

    Ok(Some(render_imported_config(
        default_project_name(start_dir),
        commands,
    )))
}

fn package_manager_template(start_dir: &Path, parsed: &serde_json::Value) -> cli::InitTemplate {
    match package_manager_name(start_dir, parsed).as_str() {
        "pnpm" => cli::InitTemplate::Pnpm,
        "yarn" => cli::InitTemplate::Yarn,
        "bun" => cli::InitTemplate::Bun,
        _ => cli::InitTemplate::Node,
    }
}

fn package_manager_name(start_dir: &Path, parsed: &serde_json::Value) -> String {
    if let Some(manager) = parsed
        .get("packageManager")
        .and_then(serde_json::Value::as_str)
    {
        if manager.starts_with("pnpm") {
            return "pnpm".to_string();
        }
        if manager.starts_with("yarn") {
            return "yarn".to_string();
        }
        if manager.starts_with("bun") {
            return "bun".to_string();
        }
    }

    if start_dir.join("pnpm-lock.yaml").is_file() {
        "pnpm".to_string()
    } else if start_dir.join("yarn.lock").is_file() {
        "yarn".to_string()
    } else if start_dir.join("bun.lockb").is_file() || start_dir.join("bun.lock").is_file() {
        "bun".to_string()
    } else {
        "npm".to_string()
    }
}

fn parse_make_targets(contents: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut targets = Vec::new();

    for line in contents.lines() {
        let line = line.trim_end();
        if line.is_empty()
            || line.starts_with('\t')
            || line.starts_with(' ')
            || line.starts_with('#')
        {
            continue;
        }

        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty()
            || name.starts_with('.')
            || name.contains('%')
            || name.contains(' ')
            || rest.trim_start().starts_with('=')
        {
            continue;
        }

        if seen.insert(name.to_string()) {
            targets.push(name.to_string());
        }
    }

    targets
}

fn parse_just_recipes(contents: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut recipes = Vec::new();

    for line in contents.lines() {
        let line = line.trim_end();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with(' ')
            || line.starts_with('\t')
        {
            continue;
        }

        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty()
            || name.starts_with('[')
            || name.starts_with("set ")
            || name.starts_with("import ")
            || name.contains(' ')
            || rest.trim_start().starts_with('=')
        {
            continue;
        }

        if seen.insert(name.to_string()) {
            recipes.push(name.to_string());
        }
    }

    recipes
}

fn find_project_file(start_dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|candidate| start_dir.join(candidate))
        .find(|path| path.is_file())
}

fn default_project_name(start_dir: &Path) -> String {
    start_dir
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .unwrap_or_else(|| "example".to_string())
}

struct ImportedCommand {
    name: String,
    program: String,
    args: Vec<String>,
    description: Option<String>,
}

fn render_imported_config(project_name: String, commands: Vec<ImportedCommand>) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "[project]\nname = {}\nroot = \".\"\n\n[commands]\n",
        toml_string(&project_name)
    ));

    let mut commands = commands;
    commands.sort_by(|left, right| left.name.cmp(&right.name));
    for command in commands {
        output.push_str(&format!(
            "{} = {{ program = {}, args = [{}]",
            toml_string(&command.name),
            toml_string(&command.program),
            command
                .args
                .iter()
                .map(|arg| toml_string(arg))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        if let Some(description) = command.description {
            output.push_str(&format!(", description = {}", toml_string(&description)));
        }
        output.push_str(" }\n");
    }

    output
}

fn toml_string(value: &str) -> String {
    format!("{:?}", value)
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
        print_stable_json(json_envelope(
            "templates",
            "ok",
            vec![
                ("count", json!(entries.len())),
                ("templates", json!(entries)),
            ],
        ));
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
    let archive_path = create_package(&config, output)?;

    if json_output {
        print_stable_json(json_envelope(
            "package",
            "ok",
            vec![
                ("output", json!(archive_path)),
                ("root", json!(config.root)),
            ],
        ));
    } else {
        println!("package: {}", archive_path.display());
    }

    Ok(0)
}

fn create_package(
    config: &config::ProjectConfig,
    output: Option<PathBuf>,
) -> Result<PathBuf, Error> {
    let archive_path = output.unwrap_or_else(|| default_package_path(config));

    if cfg!(windows) {
        create_zip_package(&config.root, &archive_path)?;
    } else {
        create_tar_gz_package(&config.root, &archive_path)?;
    }

    Ok(archive_path)
}

pub fn release_action(
    start_dir: &Path,
    output: Option<PathBuf>,
    json_output: bool,
    json_events: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let (_, config) = load_project(start_dir, profile)?;
    let started = Instant::now();

    for action in [
        Action::Build(cli::CommandArgs { args: vec![] }),
        Action::Test(cli::CommandArgs { args: vec![] }),
    ] {
        let started = Instant::now();
        let action_label = action.to_string();
        emit_json_event(
            json_events,
            "release_stage_start",
            vec![
                ("stage", json!(action_label.clone())),
                ("root", json!(config.root.clone())),
            ],
        );
        let status = runner::execute(action, &config, false, None, json_output)?;
        emit_json_event(
            json_events,
            "release_stage_finish",
            vec![
                ("stage", json!(action_label.clone())),
                ("root", json!(config.root.clone())),
                ("success", json!(status.success())),
                ("exit_code", json!(status.code())),
            ],
        );
        if !status.success() {
            print_failure_summary(
                config.name.as_deref(),
                Some(&action_label),
                status.code(),
                started.elapsed(),
            );
            return Ok(status.code().unwrap_or(1));
        }
    }

    let archive_path = create_package(&config, output)?;
    emit_json_event(
        json_events,
        "release_package_finish",
        vec![
            ("root", json!(config.root.clone())),
            ("output", json!(archive_path.clone())),
        ],
    );
    if json_output {
        print_stable_json(json_envelope(
            "release",
            "ok",
            vec![
                ("output", json!(archive_path)),
                ("root", json!(config.root)),
            ],
        ));
    } else {
        println!("package: {}", archive_path.display());
    }
    print_command_summary("release", true, 2, started.elapsed());
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
        print_stable_json(json_envelope(
            "list",
            "ok",
            vec![("commands", json!(commands))],
        ));
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
    let config_chain = discovery::discover_config_chain(start_dir)?;

    if json_output {
        print_stable_json(json_envelope(
            "which",
            "ok",
            vec![
                ("config", json!(config_path)),
                ("root", json!(config.root)),
                ("config_chain", json!(config_chain)),
                ("selected_profile", json!(config.selected_profile)),
            ],
        ));
    } else {
        println!("config: {}", config_path.display());
        println!("root: {}", config.root.display());
        println!(
            "chain: {}",
            config_chain
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(" -> ")
        );
        match config.selected_profile.as_deref() {
            Some(profile) => println!("profile: {profile}"),
            None => println!("profile: (none)"),
        }
    }

    Ok(0)
}

pub fn doctor_action(
    start_dir: &Path,
    strict: bool,
    fix: bool,
    json_output: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let (config_path, config) = load_project(start_dir, profile)?;
    let mut fixed = Vec::new();
    if fix {
        fixed = apply_doctor_fixes(&config)?;
    }
    let warnings = validation_issues(&config);
    let suggestions = doctor_suggestions(&config, &warnings);

    if json_output {
        print_stable_json(json_envelope(
            "doctor",
            if warnings.is_empty() { "ok" } else { "warn" },
            vec![
                ("config", json!(config_path)),
                ("root", json!(config.root)),
                ("warnings", json!(warnings)),
                ("suggestions", json!(suggestions)),
                ("fixed", json!(fixed)),
            ],
        ));
    } else {
        println!("config: {}", config_path.display());
        println!("root: {}", config.root.display());
        if !fixed.is_empty() {
            for item in &fixed {
                println!("fixed: {item}");
            }
        }
        if warnings.is_empty() {
            println!("status: ok");
        } else {
            for warning in &warnings {
                println!("warning: {warning}");
            }
            for suggestion in &suggestions {
                println!("suggestion: {suggestion}");
            }
        }
    }

    Ok(if strict && !warnings.is_empty() { 1 } else { 0 })
}

pub fn show_action(
    start_dir: &Path,
    name: String,
    args: Vec<String>,
    source: bool,
    json_output: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    describe_action(start_dir, name, args, source, json_output, false, profile)
}

pub fn explain_action(
    start_dir: &Path,
    name: String,
    args: Vec<String>,
    source: bool,
    json_output: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    describe_action(start_dir, name, args, source, json_output, true, profile)
}

fn describe_action(
    start_dir: &Path,
    name: String,
    args: Vec<String>,
    source: bool,
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
    let sources = command_sources(start_dir, &config, command)?;

    if json_output {
        let operation = if explain { "explain" } else { "show" };
        print_stable_json(json_envelope(
            operation,
            "ok",
            vec![
                ("config", json!(config_path)),
                ("root", json!(config.root)),
                ("name", json!(name)),
                ("rendered", json!(rendered)),
                ("cwd", json!(cwd)),
                ("timeout", json!(command.timeout())),
                ("description", json!(command.description())),
                ("shell", json!(command.is_shell())),
                ("pipeline", json!(command.is_pipeline())),
                ("source", json!(source)),
                ("sources", json!(sources)),
            ],
        ));
    } else {
        println!("name: {name}");
        println!("command: {rendered}");
        if source {
            let config_chain = discovery::discover_config_chain(start_dir)?;
            println!(
                "config chain: {}",
                config_chain
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(" -> ")
            );
            match config.selected_profile.as_deref() {
                Some(profile) => println!("selected profile: {profile}"),
                None => println!("selected profile: (none)"),
            }
        }
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
        for source in sources {
            println!("source: {source}");
        }
    }

    Ok(0)
}

fn command_sources(
    start_dir: &Path,
    config: &config::ProjectConfig,
    command: &config::CommandSpec,
) -> Result<Vec<String>, Error> {
    let mut sources = Vec::new();
    let config_paths = discovery::discover_config_chain(start_dir)?;

    for (idx, path) in config_paths.iter().enumerate() {
        if idx == 0 {
            sources.push(format!("base config: {}", path.display()));
        } else {
            sources.push(format!("child config: {}", path.display()));
        }
    }

    if let Some(profile) = config.selected_profile.as_deref() {
        sources.push(format!("profile: {profile}"));
    }

    if let Some(platform) = command.platform_override() {
        sources.push(format!("platform override: {platform}"));
    }

    Ok(sources)
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
    let started = Instant::now();
    let (command_name, args) = action_command(&action);
    let command = config
        .commands
        .get(&command_name)
        .ok_or_else(|| unknown_command_error(&command_name))?;
    enforce_safe_command(&command_name, command, safe)?;
    let rendered = command.render(&args);

    if json_output {
        print_stable_json(json_envelope(
            "dry-run",
            "ok",
            vec![
                ("config", json!(config_path)),
                ("root", json!(config.root)),
                ("name", json!(command_name)),
                ("rendered", json!(rendered)),
            ],
        ));
    } else {
        println!("[mbr] dry-run: {rendered}");
    }

    print_command_summary(
        &format!("dry-run {command_name}"),
        true,
        1,
        started.elapsed(),
    );

    Ok(0)
}

pub fn parallel_action(
    start_dir: &Path,
    names: Vec<String>,
    json_output: bool,
    json_events: bool,
    dry_run: bool,
    safe: bool,
    profile: Option<&str>,
) -> Result<i32, Error> {
    let (_, config) = load_project(start_dir, profile)?;
    trust_warning(&config);
    let started = Instant::now();

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
                    "command": name,
                    "name": name,
                    "rendered": command.render(&[]),
                    "timeout": command.timeout(),
                    "cwd": command.cwd(),
                }))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        if json_output {
            print_stable_json(json_envelope(
                "dry-run",
                "ok",
                vec![("commands", json!(commands))],
            ));
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
        let label = name.clone();
        handles.push(thread::spawn(move || {
            let started = Instant::now();
            emit_json_event(
                json_events,
                "parallel_command_start",
                vec![
                    ("command", json!(label.clone())),
                    ("project", json!(config.name.clone())),
                ],
            );
            runner::execute(
                Action::Exec(cli::ExecArgs {
                    name,
                    args: Vec::new(),
                }),
                &config,
                safe,
                Some(label.as_str()),
                json_output,
            )
            .map(|status| {
                emit_json_event(
                    json_events,
                    "parallel_command_finish",
                    vec![
                        ("command", json!(label.clone())),
                        ("project", json!(config.name.clone())),
                        ("success", json!(status.success())),
                        ("exit_code", json!(status.code())),
                    ],
                );
                (label, status, started.elapsed())
            })
        }));
    }

    let mut exit_code = 0;
    let mut errors = Vec::new();
    for handle in handles {
        match handle.join() {
            Ok(Ok((name, status, duration))) => {
                if !status.success() {
                    print_failure_summary(None, Some(&name), status.code(), duration);
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
        print_stable_json(json_envelope(
            "parallel",
            "ok",
            vec![("parallel", json!(names))],
        ));
    }

    print_command_summary("parallel", exit_code == 0, names.len(), started.elapsed());

    Ok(exit_code)
}

fn print_failure_summary(
    project: Option<&str>,
    command: Option<&str>,
    exit_code: Option<i32>,
    duration: std::time::Duration,
) {
    let mut parts = Vec::new();
    if let Some(project) = project {
        parts.push(format!("project={project}"));
    }
    if let Some(command) = command {
        parts.push(format!("command={command}"));
    }
    if let Some(code) = exit_code {
        parts.push(format!("exit={code}"));
    }
    parts.push(format!("duration={}ms", duration.as_millis()));
    eprintln!("[mbr] failed: {}", parts.join(" | "));
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
    warnings.extend(requirements_issues(config));
    warnings.extend(trust_issues(config));

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

fn trust_issues(config: &config::ProjectConfig) -> Vec<String> {
    let mut warnings = Vec::new();

    if config.trust.shell_commands {
        return warnings;
    }

    for name in config.commands.names() {
        if let Some(command) = config.commands.get(&name)
            && command.is_shell()
        {
            warnings.push(format!(
                "command `{name}` uses a shell string and is not explicitly trusted"
            ));
        }
    }

    warnings
}

fn requirements_issues(config: &config::ProjectConfig) -> Vec<String> {
    let mut warnings = Vec::new();

    for tool in &config.requirements.tools {
        if !program_on_path(tool) {
            warnings.push(format!("required tool `{tool}` was not found on PATH"));
        }
    }

    for file in &config.requirements.files {
        if !requirement_file_exists(&config.root, file) {
            warnings.push(format!("required file `{file}` was not found"));
        }
    }

    for env_name in &config.requirements.env {
        if !requirement_env_exists(&config.env, env_name) {
            warnings.push(format!("required env var `{env_name}` was not set"));
        }
    }

    warnings
}

fn doctor_suggestions(config: &config::ProjectConfig, warnings: &[String]) -> Vec<String> {
    let mut suggestions = Vec::new();

    for warning in warnings {
        if let Some(tool) = warning
            .strip_prefix("command `")
            .and_then(|rest| rest.split_once("` program `"))
            .and_then(|(_, rest)| rest.split_once('`'))
            .map(|(tool, _)| tool)
        {
            suggestions.push(format!("install `{tool}` or add it to PATH"));
        }

        if let Some(env_file) = warning
            .strip_prefix("env file `")
            .and_then(|rest| rest.split_once('`'))
            .map(|(env_file, _)| env_file)
        {
            suggestions.push(format!(
                "create `{env_file}` or update `env_file` in the config"
            ));
        }

        if let Some(env_file) = warning
            .strip_prefix("profile `")
            .and_then(|rest| rest.split_once("` env file `"))
            .and_then(|(_, rest)| rest.split_once('`'))
            .map(|(env_file, _)| env_file)
        {
            suggestions.push(format!(
                "create the profile env file `{env_file}` or remove the profile-specific `env_file`"
            ));
        }

        if let Some(tool) = warning
            .strip_prefix("required tool `")
            .and_then(|rest| rest.split_once('`'))
            .map(|(tool, _)| tool)
        {
            suggestions.push(format!("install `{tool}` or update `[requirements].tools`"));
        }

        if let Some(file) = warning
            .strip_prefix("required file `")
            .and_then(|rest| rest.split_once('`'))
            .map(|(file, _)| file)
        {
            suggestions.push(format!("create `{file}` or update `[requirements].files`"));
        }

        if let Some(env_name) = warning
            .strip_prefix("required env var `")
            .and_then(|rest| rest.split_once('`'))
            .map(|(env_name, _)| env_name)
        {
            suggestions.push(format!("set `{env_name}` or update `[requirements].env`"));
        }

        if let Some(name) = warning
            .strip_prefix("command `")
            .and_then(|rest| rest.split_once("` uses a shell string and is not explicitly trusted"))
            .map(|(name, _)| name)
        {
            suggestions.push(format!(
                "set `[trust].shell_commands = true` or convert `{name}` to a structured command"
            ));
        }
    }

    if config.name.is_none() {
        suggestions.push("set `[project].name` to make warnings easier to understand".to_string());
    }

    suggestions.sort();
    suggestions.dedup();
    suggestions
}

fn apply_doctor_fixes(config: &config::ProjectConfig) -> Result<Vec<String>, Error> {
    let mut fixed = Vec::new();

    for env_file in [
        config.env_file.as_deref(),
        config.profile_env_file.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        let path = config.root.join(env_file);
        if path.exists() {
            continue;
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::ConfigWrite {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        fs::write(&path, b"").map_err(|source| Error::ConfigWrite {
            path: path.clone(),
            source,
        })?;
        fixed.push(format!("created env file {}", path.display()));
    }

    fixed.sort();
    fixed.dedup();
    Ok(fixed)
}

fn env_file_exists(root: &Path, env_file: &str) -> bool {
    root.join(env_file).is_file()
}

fn requirement_file_exists(root: &Path, path: &str) -> bool {
    root.join(path).is_file()
}

fn requirement_env_exists(env: &std::collections::HashMap<String, String>, name: &str) -> bool {
    env.contains_key(name) || std::env::var_os(name).is_some()
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

fn print_command_summary(name: &str, success: bool, count: usize, duration: std::time::Duration) {
    let status = if success { "ok" } else { "warn" };
    eprintln!(
        "[mbr] summary: command={name} status={status} count={count} duration={}ms",
        duration.as_millis()
    );
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
        | Action::Watch(_)
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
    print_stable_json_to(&mut std::io::stdout(), value);
}

fn print_stable_json_to<W: Write>(writer: &mut W, value: Value) {
    let _ = writeln!(writer, "{}", stable_value(value));
}

fn emit_json_event(enabled: bool, event: &str, fields: Vec<(&str, Value)>) {
    if !enabled {
        return;
    }

    let mut all_fields = vec![("event", json!(event))];
    all_fields.extend(fields);
    print_stable_json_to(
        &mut std::io::stderr(),
        json_envelope("event", "ok", all_fields),
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_watch_tree_changes_when_files_change() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(temp.path().join("file.txt"), "one").expect("write file");

        let first = snapshot_watch_tree(temp.path()).expect("first snapshot");
        let second = snapshot_watch_tree(temp.path()).expect("second snapshot");
        assert_eq!(first, second);

        fs::write(temp.path().join("file.txt"), "two").expect("rewrite file");
        let third = snapshot_watch_tree(temp.path()).expect("third snapshot");
        assert_ne!(first, third);
    }
}

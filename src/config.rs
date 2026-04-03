use crate::error::Error;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectFile {
    pub project: Option<ProjectSection>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub commands: CommandsSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSection {
    pub name: Option<String>,
    pub root: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CommandsSection {
    pub build: Option<CommandSpec>,
    pub test: Option<CommandSpec>,
    pub run: Option<CommandSpec>,
    #[serde(flatten, default)]
    pub extra: HashMap<String, CommandSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CommandSpec {
    Shell(String),
    Program {
        program: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
}

impl CommandSpec {
    pub fn describe(&self, extra_args: &[String]) -> String {
        match self {
            CommandSpec::Shell(base) => {
                if extra_args.is_empty() {
                    base.clone()
                } else {
                    format!("{base} {}", render_args(extra_args))
                }
            }
            CommandSpec::Program { program, args, .. } => {
                let mut parts = vec![program.clone()];
                parts.extend(args.iter().cloned());
                parts.extend(extra_args.iter().cloned());
                parts.join(" ")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub name: Option<String>,
    pub root: PathBuf,
    pub env: HashMap<String, String>,
    pub commands: CommandsSection,
}

impl ProjectConfig {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let contents = fs::read_to_string(path).map_err(|source| Error::ConfigRead {
            path: path.to_path_buf(),
            source,
        })?;
        let file: ProjectFile = toml::from_str(&contents).map_err(|source| Error::ConfigParse {
            path: path.to_path_buf(),
            source,
        })?;

        Self::from_file(file, path)
    }

    fn from_file(file: ProjectFile, path: &Path) -> Result<Self, Error> {
        let project_dir = path.parent().unwrap_or_else(|| Path::new("."));
        let project = file.project.unwrap_or(ProjectSection {
            name: None,
            root: None,
        });

        let root = match project.root {
            Some(root) => project_dir.join(root),
            None => project_dir.to_path_buf(),
        };

        if !root.exists() || !root.is_dir() {
            return Err(Error::InvalidProjectRoot { path: root });
        }

        if file.commands.is_empty() {
            return Err(Error::MissingCommandGroup);
        }

        Ok(Self {
            name: project.name,
            root,
            env: file.env,
            commands: file.commands,
        })
    }
}

impl CommandsSection {
    pub fn is_empty(&self) -> bool {
        self.build.is_none() && self.test.is_none() && self.run.is_none() && self.extra.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&CommandSpec> {
        match name {
            "build" => self.build.as_ref(),
            "test" => self.test.as_ref(),
            "run" => self.run.as_ref(),
            _ => self.extra.get(name),
        }
    }

    pub fn names(&self) -> Vec<String> {
        let mut names = Vec::new();
        if self.build.is_some() {
            names.push("build".to_string());
        }
        if self.test.is_some() {
            names.push("test".to_string());
        }
        if self.run.is_some() {
            names.push("run".to_string());
        }
        names.extend(self.extra.keys().cloned());
        names.sort();
        names
    }
}

pub fn starter_config() -> &'static str {
    r#"[project]
name = "example"
root = "."

[env]
RUST_LOG = "info"

[commands]
build = { program = "cargo", args = ["build"] }
test = { program = "cargo", args = ["test"] }
run = { program = "cargo", args = ["run"] }
fmt = "cargo fmt --all"
clean = { program = "cargo", args = ["clean"] }
ci = "cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test"
lint = { program = "cargo", args = ["clippy", "--all-targets", "--all-features", "--", "-D", "warnings"] }
"#
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

    if arg.chars().all(|c| {
        matches!(
            c,
            'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '-' | '.' | '/' | ':' | '@' | '%' | '+' | '='
        )
    }) {
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

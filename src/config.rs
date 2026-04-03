use crate::discovery;
use crate::{cli::InitTemplate, error::Error};
use serde::{Deserialize, Deserializer, de::Error as DeError};
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
    pub profiles: HashMap<String, ProfileSection>,
    #[serde(default)]
    pub commands: CommandsSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSection {
    pub name: Option<String>,
    pub root: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProfileSection {
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub commands: CommandsSection,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CommandsSection {
    pub build: Option<CommandSpec>,
    pub test: Option<CommandSpec>,
    pub run: Option<CommandSpec>,
    #[serde(flatten, default)]
    pub extra: HashMap<String, CommandSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArgsMode {
    #[default]
    Append,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EnvMode {
    #[default]
    Merge,
    Replace,
}

#[derive(Debug, Clone)]
pub struct CommandSpec {
    extends: Option<String>,
    args_mode: ArgsMode,
    env_mode: EnvMode,
    shell: Option<String>,
    program: Option<String>,
    steps: Vec<String>,
    args: Vec<String>,
    env: HashMap<String, String>,
    cwd: Option<String>,
    timeout: Option<u64>,
    retries: Option<u32>,
    description: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct CommandOverride {
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args_mode: Option<ArgsMode>,
    #[serde(default)]
    env_mode: Option<EnvMode>,
    #[serde(default)]
    program: Option<String>,
    #[serde(default)]
    steps: Option<Vec<String>>,
    #[serde(default)]
    args: Option<Vec<String>>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    timeout: Option<u64>,
    #[serde(default)]
    retries: Option<u32>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
enum RawCommandSpec {
    Shell(String),
    Table {
        #[serde(default)]
        extends: Option<String>,
        #[serde(default)]
        command: Option<String>,
        #[serde(default)]
        args_mode: ArgsMode,
        #[serde(default)]
        env_mode: EnvMode,
        #[serde(default)]
        program: Option<String>,
        #[serde(default)]
        steps: Vec<String>,
        #[serde(default)]
        windows: Option<CommandOverride>,
        #[serde(default)]
        unix: Option<CommandOverride>,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        timeout: Option<u64>,
        #[serde(default)]
        retries: Option<u32>,
        #[serde(default)]
        description: Option<String>,
    },
}

impl<'de> Deserialize<'de> for CommandSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawCommandSpec::deserialize(deserializer)?;
        let (mut spec, windows, unix) = match raw {
            RawCommandSpec::Shell(command) => (
                Self {
                    extends: None,
                    args_mode: ArgsMode::Append,
                    env_mode: EnvMode::Merge,
                    shell: Some(command),
                    program: None,
                    steps: vec![],
                    args: vec![],
                    env: HashMap::new(),
                    cwd: None,
                    timeout: None,
                    retries: None,
                    description: None,
                },
                None,
                None,
            ),
            RawCommandSpec::Table {
                extends,
                command,
                args_mode,
                env_mode,
                program,
                steps,
                windows,
                unix,
                args,
                env,
                cwd,
                timeout,
                retries,
                description,
            } => (
                Self {
                    extends,
                    args_mode,
                    env_mode,
                    shell: command,
                    program,
                    steps,
                    args,
                    env,
                    cwd,
                    timeout,
                    retries,
                    description,
                },
                windows,
                unix,
            ),
        };

        if cfg!(windows) {
            if let Some(override_spec) = windows {
                spec.apply_override(&override_spec);
            }
        } else if let Some(override_spec) = unix {
            spec.apply_override(&override_spec);
        }

        if spec.shell.is_none()
            && spec.program.is_none()
            && spec.steps.is_empty()
            && spec.extends.is_none()
        {
            return Err(D::Error::custom(
                "command table must define `program`, `command`, `steps`, or `extends`",
            ));
        }

        Ok(spec)
    }
}

impl CommandSpec {
    pub fn render(&self, extra_args: &[String]) -> String {
        if self.is_pipeline() {
            return self.steps.join(" -> ");
        }

        match (self.program.as_deref(), self.shell.as_deref()) {
            (None, Some(base)) => {
                if extra_args.is_empty() {
                    base.to_string()
                } else {
                    format!("{base} {}", render_args(extra_args))
                }
            }
            (Some(program), None) => {
                let mut parts = vec![program.to_string()];
                parts.extend(self.args.iter().map(|arg| quote_arg(arg)));
                parts.extend(extra_args.iter().map(|arg| quote_arg(arg)));
                parts.join(" ")
            }
            _ => String::new(),
        }
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    pub fn timeout(&self) -> Option<u64> {
        self.timeout
    }

    pub fn retries(&self) -> Option<u32> {
        self.retries
    }

    pub fn is_shell(&self) -> bool {
        self.shell.is_some()
    }

    pub fn program(&self) -> Option<&str> {
        self.program.as_deref()
    }

    pub fn extends(&self) -> Option<&str> {
        self.extends.as_deref()
    }

    pub fn steps(&self) -> &[String] {
        &self.steps
    }

    pub fn is_pipeline(&self) -> bool {
        !self.steps.is_empty()
    }

    pub fn shell_command(&self) -> Option<&str> {
        self.shell.as_deref()
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn env(&self) -> &HashMap<String, String> {
        &self.env
    }

    fn apply_override(&mut self, override_spec: &CommandOverride) {
        if let Some(command) = &override_spec.command {
            self.shell = Some(command.clone());
            self.program = None;
            self.steps.clear();
        }

        if let Some(program) = &override_spec.program {
            self.program = Some(program.clone());
            self.shell = None;
            self.steps.clear();
        }

        if let Some(steps) = &override_spec.steps {
            self.steps = steps.clone();
            self.program = None;
            self.shell = None;
        }

        if let Some(args_mode) = override_spec.args_mode {
            self.args_mode = args_mode;
        }

        if let Some(env_mode) = override_spec.env_mode {
            self.env_mode = env_mode;
        }

        if let Some(args) = &override_spec.args {
            self.args = args.clone();
        }

        if let Some(env) = &override_spec.env {
            self.env = env.clone();
        }

        if let Some(cwd) = &override_spec.cwd {
            self.cwd = Some(cwd.clone());
        }

        if let Some(timeout) = override_spec.timeout {
            self.timeout = Some(timeout);
        }

        if let Some(retries) = override_spec.retries {
            self.retries = Some(retries);
        }

        if let Some(description) = &override_spec.description {
            self.description = Some(description.clone());
        }
    }

    fn merge_from(&self, base: &CommandSpec) -> CommandSpec {
        let mut merged = base.clone();

        if self.shell.is_some() {
            merged.shell = self.shell.clone();
            merged.program = None;
            merged.steps.clear();
        }

        if self.program.is_some() {
            merged.program = self.program.clone();
            merged.shell = None;
            merged.steps.clear();
        }

        if !self.steps.is_empty() {
            merged.steps = self.steps.clone();
            merged.program = None;
            merged.shell = None;
        }

        match self.args_mode {
            ArgsMode::Append => {
                if !self.args.is_empty() {
                    merged.args.extend(self.args.clone());
                }
            }
            ArgsMode::Replace => {
                merged.args = self.args.clone();
            }
        }

        match self.env_mode {
            EnvMode::Merge => {
                if !self.env.is_empty() {
                    merged.env.extend(self.env.clone());
                }
            }
            EnvMode::Replace => {
                merged.env = self.env.clone();
            }
        }

        if self.cwd.is_some() {
            merged.cwd = self.cwd.clone();
        }

        if self.timeout.is_some() {
            merged.timeout = self.timeout;
        }

        if self.retries.is_some() {
            merged.retries = self.retries;
        }

        if self.description.is_some() {
            merged.description = self.description.clone();
        }

        merged.extends = None;
        merged
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
    #[allow(dead_code)]
    pub fn load(path: &Path) -> Result<Self, Error> {
        let file = load_file(path)?;
        Self::from_file(file, path)
    }

    pub fn load_inherited(start: &Path) -> Result<Self, Error> {
        let config_paths = discovery::discover_config_chain(start)?;
        let mut name = None;
        let mut root: Option<PathBuf> = None;
        let mut env = HashMap::new();
        let mut commands = CommandsSection::default();
        let mut profiles: HashMap<String, ProfileSection> = HashMap::new();

        for path in config_paths {
            let file = load_file(&path)?;
            let project_dir = path.parent().unwrap_or_else(|| Path::new("."));

            if let Some(project) = file.project {
                if let Some(value) = project.name {
                    name = Some(value);
                }

                if let Some(value) = project.root {
                    root = Some(resolve_root(project_dir, &value));
                } else if root.is_none() {
                    root = Some(project_dir.to_path_buf());
                }
            } else if root.is_none() {
                root = Some(project_dir.to_path_buf());
            }

            env.extend(file.env);
            merge_profiles(&mut profiles, file.profiles);
            commands.merge_from(file.commands);
        }

        apply_selected_profile(&mut env, &mut commands, &profiles)?;

        commands = commands.resolve_inheritance()?;

        let root = root.ok_or_else(|| Error::ConfigNotFound {
            start: start.to_path_buf(),
        })?;

        if !root.exists() || !root.is_dir() {
            return Err(Error::InvalidProjectRoot { path: root });
        }

        load_env_file(&root, &mut env)?;

        if commands.is_empty() {
            return Err(Error::MissingCommandGroup);
        }

        Ok(Self {
            name,
            root,
            env,
            commands,
        })
    }

    #[allow(dead_code)]
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

        let mut env = file.env;
        load_env_file(&root, &mut env)?;

        if file.commands.is_empty() {
            return Err(Error::MissingCommandGroup);
        }
        let mut commands = file.commands;
        apply_selected_profile(&mut env, &mut commands, &file.profiles)?;

        Ok(Self {
            name: project.name,
            root,
            env,
            commands: commands.resolve_inheritance()?,
        })
    }
}

fn load_file(path: &Path) -> Result<ProjectFile, Error> {
    let contents = fs::read_to_string(path).map_err(|source| Error::ConfigRead {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&contents).map_err(|source| Error::ConfigParse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })
}

fn resolve_root(project_dir: &Path, root: &str) -> PathBuf {
    let root = Path::new(root);
    if root.is_absolute() {
        root.to_path_buf()
    } else {
        project_dir.join(root)
    }
}

fn load_env_file(root: &Path, env: &mut HashMap<String, String>) -> Result<(), Error> {
    let path = root.join(".env");
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(Error::ConfigRead { path, source }),
    };

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let key = key.trim();
        if key.is_empty() || env.contains_key(key) {
            continue;
        }

        env.insert(key.to_string(), parse_env_value(value.trim()));
    }

    Ok(())
}

fn parse_env_value(value: &str) -> String {
    if let Some(inner) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        return inner.replace("\\n", "\n").replace("\\t", "\t");
    }

    if let Some(inner) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        return inner.to_string();
    }

    value.to_string()
}

fn merge_profiles(
    target: &mut HashMap<String, ProfileSection>,
    source: HashMap<String, ProfileSection>,
) {
    for (name, profile) in source {
        target
            .entry(name)
            .and_modify(|existing| {
                existing.env.extend(profile.env.clone());
                existing.commands.merge_from(profile.commands.clone());
            })
            .or_insert(profile);
    }
}

fn apply_selected_profile(
    env: &mut HashMap<String, String>,
    commands: &mut CommandsSection,
    profiles: &HashMap<String, ProfileSection>,
) -> Result<(), Error> {
    let Some(profile_name) = std::env::var("MBR_PROFILE")
        .ok()
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };

    let Some(profile) = profiles.get(&profile_name) else {
        return Err(Error::UnknownProfile { name: profile_name });
    };

    env.extend(profile.env.clone());
    commands.merge_from(profile.commands.clone());
    Ok(())
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

    pub fn merge_from(&mut self, other: CommandsSection) {
        if other.build.is_some() {
            self.build = other.build;
        }
        if other.test.is_some() {
            self.test = other.test;
        }
        if other.run.is_some() {
            self.run = other.run;
        }
        for (name, command) in other.extra {
            self.extra.insert(name, command);
        }
    }

    pub fn resolve_inheritance(&self) -> Result<Self, Error> {
        let map = self.to_map();
        let mut resolved = HashMap::new();

        let mut names: Vec<_> = map.keys().cloned().collect();
        names.sort();

        for name in names {
            let spec = resolve_command(&name, &map, &mut resolved, &mut Vec::new())?;
            resolved.insert(name, spec);
        }

        Ok(Self::from_map(resolved))
    }

    fn to_map(&self) -> HashMap<String, CommandSpec> {
        let mut map = HashMap::new();
        if let Some(command) = &self.build {
            map.insert("build".to_string(), command.clone());
        }
        if let Some(command) = &self.test {
            map.insert("test".to_string(), command.clone());
        }
        if let Some(command) = &self.run {
            map.insert("run".to_string(), command.clone());
        }
        for (name, command) in &self.extra {
            map.insert(name.clone(), command.clone());
        }
        map
    }

    fn from_map(mut map: HashMap<String, CommandSpec>) -> Self {
        let build = map.remove("build");
        let test = map.remove("test");
        let run = map.remove("run");
        Self {
            build,
            test,
            run,
            extra: map,
        }
    }
}

fn resolve_command(
    name: &str,
    source: &HashMap<String, CommandSpec>,
    resolved: &mut HashMap<String, CommandSpec>,
    stack: &mut Vec<String>,
) -> Result<CommandSpec, Error> {
    if let Some(spec) = resolved.get(name) {
        return Ok(spec.clone());
    }

    if stack.iter().any(|entry| entry == name) {
        return Err(Error::CommandInheritanceCycle {
            name: name.to_string(),
        });
    }

    let spec = source
        .get(name)
        .ok_or_else(|| Error::UnknownCommand {
            name: name.to_string(),
        })?
        .clone();

    if let Some(base_name) = spec.extends() {
        stack.push(name.to_string());
        let base =
            resolve_command(base_name, source, resolved, stack).map_err(|err| match err {
                Error::UnknownCommand { .. } => Error::UnknownCommandBase {
                    name: name.to_string(),
                    base: base_name.to_string(),
                },
                other => other,
            })?;
        stack.pop();
        let merged = spec.merge_from(&base);
        resolved.insert(name.to_string(), merged.clone());
        Ok(merged)
    } else {
        resolved.insert(name.to_string(), spec.clone());
        Ok(spec)
    }
}

pub fn starter_config_for(template: InitTemplate) -> &'static str {
    match template {
        InitTemplate::Rust => {
            r#"[project]
name = "example"
root = "."

[env]
RUST_LOG = "info"

[commands]
build = { program = "cargo", args = ["build"], description = "Compile the project" }
test = { program = "cargo", args = ["test"], description = "Run tests" }
run = { program = "cargo", args = ["run"], description = "Run the app" }
fmt = { program = "cargo", args = ["fmt", "--all"], description = "Format source files" }
clean = { program = "cargo", args = ["clean"], description = "Remove build artifacts" }
ci = "cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test"
lint = { program = "cargo", args = ["clippy", "--all-targets", "--all-features", "--", "-D", "warnings"], description = "Run Clippy" }
"#
        }
        InitTemplate::Node => {
            r#"[project]
name = "example"
root = "."

[commands]
build = { program = "npm", args = ["run", "build"], description = "Build the app" }
test = { program = "npm", args = ["test"], description = "Run tests" }
run = { program = "npm", args = ["start"], description = "Start the app" }
fmt = { program = "npm", args = ["run", "format"], description = "Format files" }
clean = { program = "npm", args = ["run", "clean"], description = "Remove generated files" }
ci = "npm run ci"
"#
        }
        InitTemplate::Pnpm => {
            r#"[project]
name = "example"
root = "."

[commands]
build = { program = "pnpm", args = ["run", "build"], description = "Build the app" }
test = { program = "pnpm", args = ["test"], description = "Run tests" }
run = { program = "pnpm", args = ["start"], description = "Start the app" }
fmt = { program = "pnpm", args = ["run", "format"], description = "Format files" }
clean = { program = "pnpm", args = ["run", "clean"], description = "Remove generated files" }
ci = "pnpm run ci"
"#
        }
        InitTemplate::Yarn => {
            r#"[project]
name = "example"
root = "."

[commands]
build = { program = "yarn", args = ["build"], description = "Build the app" }
test = { program = "yarn", args = ["test"], description = "Run tests" }
run = { program = "yarn", args = ["start"], description = "Start the app" }
fmt = { program = "yarn", args = ["format"], description = "Format files" }
clean = { program = "yarn", args = ["clean"], description = "Remove generated files" }
ci = "yarn ci"
"#
        }
        InitTemplate::Python => {
            r#"[project]
name = "example"
root = "."

[commands]
build = { program = "python", args = ["-m", "build"], description = "Build the package" }
test = { program = "pytest", args = [], description = "Run tests" }
run = { program = "python", args = ["main.py"], description = "Run the app" }
fmt = { program = "ruff", args = ["format", "."], description = "Format source files" }
clean = { program = "python", args = ["-c", "import shutil; [shutil.rmtree(p, ignore_errors=True) for p in ('build', 'dist')]"], description = "Remove build outputs" }
ci = "pytest && ruff check . && python -m build"
"#
        }
        InitTemplate::Poetry => {
            r#"[project]
name = "example"
root = "."

[commands]
build = { program = "poetry", args = ["build"], description = "Build the package" }
test = { program = "poetry", args = ["run", "pytest"], description = "Run tests" }
run = { program = "poetry", args = ["run", "python", "main.py"], description = "Run the app" }
fmt = { program = "poetry", args = ["run", "ruff", "format", "."], description = "Format source files" }
clean = { program = "poetry", args = ["run", "python", "-c", "import shutil; [shutil.rmtree(p, ignore_errors=True) for p in ('build', 'dist')]"], description = "Remove build outputs" }
ci = "poetry run pytest && poetry run ruff check . && poetry build"
"#
        }
        InitTemplate::Uv => {
            r#"[project]
name = "example"
root = "."

[commands]
build = { program = "uv", args = ["build"], description = "Build the package" }
test = { program = "uv", args = ["run", "pytest"], description = "Run tests" }
run = { program = "uv", args = ["run", "python", "main.py"], description = "Run the app" }
fmt = { program = "uv", args = ["run", "ruff", "format", "."], description = "Format source files" }
clean = { program = "uv", args = ["run", "python", "-c", "import shutil; [shutil.rmtree(p, ignore_errors=True) for p in ('build', 'dist')]"], description = "Remove build outputs" }
ci = "uv run pytest && uv run ruff check . && uv build"
"#
        }
        InitTemplate::Go => {
            r#"[project]
name = "example"
root = "."

[commands]
build = { program = "go", args = ["build", "./..."], description = "Build all packages" }
test = { program = "go", args = ["test", "./..."], description = "Run tests" }
run = { program = "go", args = ["run", "."], description = "Run the app" }
fmt = { program = "gofmt", args = ["-w", "."], description = "Format source files" }
clean = { program = "go", args = ["clean"], description = "Clean build cache" }
ci = "go test ./..."
"#
        }
        InitTemplate::CargoWorkspace => {
            r#"[project]
name = "example"
root = "."

[commands]
build = { program = "cargo", args = ["build", "--workspace"], description = "Build workspace packages" }
test = { program = "cargo", args = ["test", "--workspace"], description = "Run workspace tests" }
run = { program = "cargo", args = ["run", "--workspace"], description = "Run the selected package" }
fmt = { program = "cargo", args = ["fmt", "--all"], description = "Format source files" }
clean = { program = "cargo", args = ["clean"], description = "Remove build artifacts" }
ci = "cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace"
"#
        }
        InitTemplate::Cmake => {
            r#"[project]
name = "example"
root = "."

[commands]
build = { program = "cmake", args = ["-S", ".", "-B", "build"], description = "Configure the build" }
test = { program = "ctest", args = ["--test-dir", "build"], description = "Run tests" }
run = { program = "cmake", args = ["--build", "build", "--target", "run"], description = "Replace with your executable target" }
fmt = { program = "cmake-format", args = ["-i", "CMakeLists.txt"], description = "Format CMake files" }
clean = { program = "cmake", args = ["--build", "build", "--target", "clean"], description = "Remove build outputs" }
ci = "ctest --test-dir build"
"#
        }
        InitTemplate::CmakeNinja => {
            r#"[project]
name = "example"
root = "."

[commands]
build = { program = "cmake", args = ["-S", ".", "-B", "build", "-G", "Ninja"], description = "Configure the build" }
test = { program = "ctest", args = ["--test-dir", "build"], description = "Run tests" }
run = { program = "cmake", args = ["--build", "build", "--target", "run"], description = "Replace with your executable target" }
fmt = { program = "cmake-format", args = ["-i", "CMakeLists.txt"], description = "Format CMake files" }
clean = { program = "cmake", args = ["--build", "build", "--target", "clean"], description = "Remove build outputs" }
ci = "ctest --test-dir build"
"#
        }
        InitTemplate::Generic => {
            r#"[project]
name = "example"
root = "."

[commands]
build = "echo build"
test = "echo test"
run = "echo run"
fmt = "echo fmt"
clean = "echo clean"
ci = "echo ci"
"#
        }
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

fn quote_arg(arg: &str) -> String {
    if arg.is_empty() {
        return if cfg!(windows) {
            "\"\"".to_string()
        } else {
            "''".to_string()
        };
    }

    if cfg!(windows) {
        windows_quote(arg)
    } else {
        unix_quote(arg)
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

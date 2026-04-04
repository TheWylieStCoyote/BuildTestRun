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
    template_spec(template).body
}

pub fn template_spec(template: InitTemplate) -> &'static TemplateSpec {
    match template {
        InitTemplate::Rust => &TEMPLATE_RUST,
        InitTemplate::Node => &TEMPLATE_NODE,
        InitTemplate::Pnpm => &TEMPLATE_PNPM,
        InitTemplate::Yarn => &TEMPLATE_YARN,
        InitTemplate::Bun => &TEMPLATE_BUN,
        InitTemplate::Deno => &TEMPLATE_DENO,
        InitTemplate::Nextjs => &TEMPLATE_NEXTJS,
        InitTemplate::Vite => &TEMPLATE_VITE,
        InitTemplate::Turbo => &TEMPLATE_TURBO,
        InitTemplate::Nx => &TEMPLATE_NX,
        InitTemplate::Python => &TEMPLATE_PYTHON,
        InitTemplate::Django => &TEMPLATE_DJANGO,
        InitTemplate::Fastapi => &TEMPLATE_FASTAPI,
        InitTemplate::Flask => &TEMPLATE_FLASK,
        InitTemplate::Poetry => &TEMPLATE_POETRY,
        InitTemplate::Hatch => &TEMPLATE_HATCH,
        InitTemplate::Pixi => &TEMPLATE_PIXI,
        InitTemplate::Uv => &TEMPLATE_UV,
        InitTemplate::Go => &TEMPLATE_GO,
        InitTemplate::CargoWorkspace => &TEMPLATE_CARGO_WORKSPACE,
        InitTemplate::JavaGradle => &TEMPLATE_JAVA_GRADLE,
        InitTemplate::JavaMaven => &TEMPLATE_JAVA_MAVEN,
        InitTemplate::KotlinGradle => &TEMPLATE_KOTLIN_GRADLE,
        InitTemplate::Dotnet => &TEMPLATE_DOTNET,
        InitTemplate::PhpComposer => &TEMPLATE_PHP_COMPOSER,
        InitTemplate::RubyBundler => &TEMPLATE_RUBY_BUNDLER,
        InitTemplate::Rails => &TEMPLATE_RAILS,
        InitTemplate::Laravel => &TEMPLATE_LARAVEL,
        InitTemplate::Terraform => &TEMPLATE_TERRAFORM,
        InitTemplate::Helm => &TEMPLATE_HELM,
        InitTemplate::DockerCompose => &TEMPLATE_DOCKER_COMPOSE,
        InitTemplate::Cmake => &TEMPLATE_CMAKE,
        InitTemplate::CmakeNinja => &TEMPLATE_CMAKE_NINJA,
        InitTemplate::Generic => &TEMPLATE_GENERIC,
    }
}

pub struct TemplateSpec {
    pub body: &'static str,
    pub warning: &'static str,
}

const TEMPLATE_RUST: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[env]
RUST_LOG = "info"

[commands]
build = { program = "cargo", args = ["build"], description = "Compile the project" }
test = { program = "cargo", args = ["test"], description = "Run tests" }
run = { program = "cargo", args = ["run"], description = "Run the app" }
fmt = { program = "cargo", args = ["fmt", "--all"], description = "Format source files" }
docs = { program = "cargo", args = ["doc"], description = "Generate documentation" }
clean = { program = "cargo", args = ["clean"], description = "Remove build artifacts" }
ci = { steps = ["fmt", "lint", "test"], description = "Run the standard checks" }
lint = { program = "cargo", args = ["clippy", "--all-targets", "--all-features", "--", "-D", "warnings"], description = "Run Clippy" }
"#,
    warning: "Rust starter uses cargo shell-based ci; review before CI use",
};

const TEMPLATE_NODE: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "npm", args = ["run", "build"], description = "Build the app" }
test = { program = "npm", args = ["test"], description = "Run tests" }
run = { program = "npm", args = ["start"], description = "Start the app" }
dev = { program = "npm", args = ["run", "dev"], description = "Start the dev server" }
fmt = { program = "npm", args = ["run", "format"], description = "Format files" }
lint = { program = "npm", args = ["run", "lint"], description = "Run lint checks" }
typecheck = { program = "npm", args = ["run", "typecheck"], description = "Run TypeScript checks" }
clean = { program = "npm", args = ["run", "clean"], description = "Remove generated files" }
ci = { steps = ["fmt", "lint", "typecheck", "test"], description = "Run the standard checks" }
"#,
    warning: "Node starter uses npm script conventions; ensure scripts exist",
};

const TEMPLATE_PNPM: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "pnpm", args = ["run", "build"], description = "Build the app" }
test = { program = "pnpm", args = ["test"], description = "Run tests" }
run = { program = "pnpm", args = ["start"], description = "Start the app" }
dev = { program = "pnpm", args = ["run", "dev"], description = "Start the dev server" }
fmt = { program = "pnpm", args = ["run", "format"], description = "Format files" }
lint = { program = "pnpm", args = ["run", "lint"], description = "Run lint checks" }
typecheck = { program = "pnpm", args = ["run", "typecheck"], description = "Run TypeScript checks" }
clean = { program = "pnpm", args = ["run", "clean"], description = "Remove generated files" }
ci = { steps = ["fmt", "lint", "typecheck", "test"], description = "Run the standard checks" }
"#,
    warning: "pnpm starter assumes pnpm scripts exist in package.json",
};

const TEMPLATE_YARN: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "yarn", args = ["build"], description = "Build the app" }
test = { program = "yarn", args = ["test"], description = "Run tests" }
run = { program = "yarn", args = ["start"], description = "Start the app" }
dev = { program = "yarn", args = ["dev"], description = "Start the dev server" }
fmt = { program = "yarn", args = ["format"], description = "Format files" }
lint = { program = "yarn", args = ["lint"], description = "Run lint checks" }
typecheck = { program = "yarn", args = ["typecheck"], description = "Run TypeScript checks" }
clean = { program = "yarn", args = ["clean"], description = "Remove generated files" }
ci = { steps = ["fmt", "lint", "typecheck", "test"], description = "Run the standard checks" }
"#,
    warning: "Yarn starter assumes yarn scripts exist in package.json",
};

const TEMPLATE_BUN: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "bun", args = ["run", "build"], description = "Build the app" }
test = { program = "bun", args = ["test"], description = "Run tests" }
run = { program = "bun", args = ["run", "start"], description = "Start the app" }
dev = { program = "bun", args = ["run", "dev"], description = "Start the dev server" }
fmt = { program = "bunx", args = ["prettier", "--write", "."], description = "Format files" }
lint = { program = "bun", args = ["run", "lint"], description = "Run lint checks" }
typecheck = { program = "bunx", args = ["tsc", "--noEmit"], description = "Run TypeScript checks" }
clean = { program = "bun", args = ["run", "clean"], description = "Remove generated files" }
ci = { steps = ["fmt", "lint", "typecheck", "test"], description = "Run the standard checks" }
"#,
    warning: "Bun starter assumes Bun scripts exist in package.json",
};

const TEMPLATE_DENO: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "deno", args = ["task", "build"], description = "Build the app" }
test = { program = "deno", args = ["test"], description = "Run tests" }
run = { program = "deno", args = ["task", "start"], description = "Start the app" }
dev = { program = "deno", args = ["task", "dev"], description = "Start the dev server" }
fmt = { program = "deno", args = ["fmt"], description = "Format files" }
lint = { program = "deno", args = ["lint"], description = "Run lint checks" }
check = { program = "deno", args = ["check", "main.ts"], description = "Run type checks" }
clean = { program = "deno", args = ["task", "clean"], description = "Remove generated files" }
ci = { steps = ["fmt", "lint", "check", "test"], description = "Run the standard checks" }
"#,
    warning: "Deno starter assumes Deno tasks and entrypoints are configured",
};

const TEMPLATE_NEXTJS: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "npm", args = ["run", "build"], description = "Build the app" }
test = { program = "npm", args = ["test"], description = "Run tests" }
run = { program = "npm", args = ["run", "start"], description = "Start the app" }
dev = { program = "npm", args = ["run", "dev"], description = "Start the dev server" }
fmt = { program = "npm", args = ["run", "format"], description = "Format files" }
lint = { program = "npm", args = ["run", "lint"], description = "Run lint checks" }
typecheck = { program = "npm", args = ["run", "typecheck"], description = "Run TypeScript checks" }
clean = { program = "npm", args = ["run", "clean"], description = "Remove generated files" }
ci = { steps = ["fmt", "lint", "typecheck", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Next.js starter assumes npm scripts exist and match the defaults",
};

const TEMPLATE_VITE: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "npm", args = ["run", "build"], description = "Build the app" }
test = { program = "npm", args = ["test"], description = "Run tests" }
run = { program = "npm", args = ["run", "preview"], description = "Preview the app" }
dev = { program = "npm", args = ["run", "dev"], description = "Start the dev server" }
fmt = { program = "npm", args = ["run", "format"], description = "Format files" }
lint = { program = "npm", args = ["run", "lint"], description = "Run lint checks" }
typecheck = { program = "npm", args = ["run", "typecheck"], description = "Run TypeScript checks" }
clean = { program = "npm", args = ["run", "clean"], description = "Remove generated files" }
ci = { steps = ["fmt", "lint", "typecheck", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Vite starter assumes npm scripts exist and match the defaults",
};

const TEMPLATE_TURBO: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "turbo", args = ["run", "build"], description = "Build workspace packages" }
test = { program = "turbo", args = ["run", "test"], description = "Run workspace tests" }
run = { program = "turbo", args = ["run", "start"], description = "Start the app" }
dev = { program = "turbo", args = ["run", "dev"], description = "Start the dev server" }
fmt = { program = "turbo", args = ["run", "format"], description = "Format files" }
lint = { program = "turbo", args = ["run", "lint"], description = "Run lint checks" }
typecheck = { program = "turbo", args = ["run", "typecheck"], description = "Run TypeScript checks" }
clean = { program = "turbo", args = ["run", "clean"], description = "Remove generated files" }
ci = { steps = ["fmt", "lint", "typecheck", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Turbo starter assumes workspace scripts exist and match the defaults",
};

const TEMPLATE_NX: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "nx", args = ["run-many", "-t", "build"], description = "Build workspace packages" }
test = { program = "nx", args = ["run-many", "-t", "test"], description = "Run workspace tests" }
run = { program = "nx", args = ["run-many", "-t", "serve"], description = "Serve the app" }
dev = { program = "nx", args = ["run-many", "-t", "serve"], description = "Start the dev server" }
fmt = { program = "nx", args = ["format:write"], description = "Format files" }
lint = { program = "nx", args = ["run-many", "-t", "lint"], description = "Run lint checks" }
typecheck = { program = "nx", args = ["run-many", "-t", "typecheck"], description = "Run TypeScript checks" }
clean = { program = "nx", args = ["reset"], description = "Reset workspace caches" }
ci = { steps = ["fmt", "lint", "typecheck", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Nx starter assumes workspace targets exist and match the defaults",
};

const TEMPLATE_DJANGO: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "python", args = ["-m", "build"], description = "Build the package" }
test = { program = "python", args = ["manage.py", "test"], description = "Run tests" }
run = { program = "python", args = ["manage.py", "runserver"], description = "Start the dev server" }
dev = { program = "python", args = ["manage.py", "runserver"], description = "Start the dev server" }
fmt = { program = "ruff", args = ["format", "."], description = "Format source files" }
lint = { program = "ruff", args = ["check", "."], description = "Run lint checks" }
check = { program = "python", args = ["manage.py", "check"], description = "Run Django checks" }
clean = { program = "python", args = ["-c", "import shutil; [shutil.rmtree(p, ignore_errors=True) for p in ('build', 'dist')]"], description = "Remove build outputs" }
ci = { steps = ["fmt", "lint", "check", "test"], description = "Run the standard checks" }
"#,
    warning: "Django starter assumes manage.py, pytest, and ruff are installed",
};

const TEMPLATE_FASTAPI: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "python", args = ["-m", "build"], description = "Build the package" }
test = { program = "pytest", args = [], description = "Run tests" }
run = { program = "uvicorn", args = ["main:app", "--reload"], description = "Run the ASGI app" }
dev = { program = "uvicorn", args = ["main:app", "--reload"], description = "Run the ASGI app" }
fmt = { program = "ruff", args = ["format", "."], description = "Format source files" }
lint = { program = "ruff", args = ["check", "."], description = "Run lint checks" }
typecheck = { program = "mypy", args = ["."], description = "Run static type checks" }
clean = { program = "python", args = ["-c", "import shutil; [shutil.rmtree(p, ignore_errors=True) for p in ('build', 'dist')]"], description = "Remove build outputs" }
ci = { steps = ["fmt", "lint", "typecheck", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "FastAPI starter assumes main:app and Python tooling are installed",
};

const TEMPLATE_FLASK: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "python", args = ["-m", "build"], description = "Build the package" }
test = { program = "pytest", args = [], description = "Run tests" }
run = { program = "flask", args = ["--app", "app", "run", "--debug"], description = "Run the Flask app" }
dev = { program = "flask", args = ["--app", "app", "run", "--debug"], description = "Run the Flask app" }
fmt = { program = "ruff", args = ["format", "."], description = "Format source files" }
lint = { program = "ruff", args = ["check", "."], description = "Run lint checks" }
typecheck = { program = "mypy", args = ["."], description = "Run static type checks" }
clean = { program = "python", args = ["-c", "import shutil; [shutil.rmtree(p, ignore_errors=True) for p in ('build', 'dist')]"], description = "Remove build outputs" }
ci = { steps = ["fmt", "lint", "typecheck", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Flask starter assumes app.py and Python tooling are installed",
};

const TEMPLATE_HATCH: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "hatch", args = ["build"], description = "Build the package" }
test = { program = "hatch", args = ["test"], description = "Run tests" }
run = { program = "hatch", args = ["run", "python", "main.py"], description = "Run the app" }
dev = { program = "hatch", args = ["run", "python", "main.py"], description = "Run the local app" }
fmt = { program = "hatch", args = ["run", "ruff", "format", "."], description = "Format source files" }
lint = { program = "hatch", args = ["run", "ruff", "check", "."], description = "Run lint checks" }
typecheck = { program = "hatch", args = ["run", "mypy", "."], description = "Run static type checks" }
clean = { program = "python", args = ["-c", "import shutil; [shutil.rmtree(p, ignore_errors=True) for p in ('build', 'dist')]"], description = "Remove build outputs" }
ci = { steps = ["fmt", "lint", "typecheck", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Hatch starter assumes hatch and Python tooling are installed",
};

const TEMPLATE_PIXI: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "pixi", args = ["run", "build"], description = "Build the package" }
test = { program = "pixi", args = ["run", "test"], description = "Run tests" }
run = { program = "pixi", args = ["run", "start"], description = "Run the app" }
dev = { program = "pixi", args = ["run", "dev"], description = "Start the dev server" }
fmt = { program = "pixi", args = ["run", "fmt"], description = "Format source files" }
lint = { program = "pixi", args = ["run", "lint"], description = "Run lint checks" }
typecheck = { program = "pixi", args = ["run", "typecheck"], description = "Run static type checks" }
clean = { program = "pixi", args = ["run", "clean"], description = "Remove build outputs" }
ci = { steps = ["fmt", "lint", "typecheck", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Pixi starter assumes project tasks exist and match the defaults",
};

const TEMPLATE_JAVA_GRADLE: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "./gradlew", args = ["build"], windows = { program = "gradlew.bat", args = ["build"] }, description = "Build the project" }
test = { program = "./gradlew", args = ["test"], windows = { program = "gradlew.bat", args = ["test"] }, description = "Run tests" }
run = { program = "./gradlew", args = ["run"], windows = { program = "gradlew.bat", args = ["run"] }, description = "Run the app" }
dev = { program = "./gradlew", args = ["run"], windows = { program = "gradlew.bat", args = ["run"] }, description = "Run the app" }
fmt = { program = "./gradlew", args = ["spotlessApply"], windows = { program = "gradlew.bat", args = ["spotlessApply"] }, description = "Format source files" }
lint = { program = "./gradlew", args = ["check"], windows = { program = "gradlew.bat", args = ["check"] }, description = "Run lint and checks" }
check = { program = "./gradlew", args = ["check"], windows = { program = "gradlew.bat", args = ["check"] }, description = "Run checks" }
clean = { program = "./gradlew", args = ["clean"], windows = { program = "gradlew.bat", args = ["clean"] }, description = "Remove build outputs" }
ci = { steps = ["fmt", "lint", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Gradle starter assumes a wrapper script and configured plugins",
};

const TEMPLATE_JAVA_MAVEN: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "mvn", args = ["package"], description = "Build the project" }
test = { program = "mvn", args = ["test"], description = "Run tests" }
run = { program = "mvn", args = ["exec:java"], description = "Run the app" }
dev = { program = "mvn", args = ["exec:java"], description = "Run the app" }
fmt = { program = "mvn", args = ["spotless:apply"], description = "Format source files" }
lint = { program = "mvn", args = ["verify"], description = "Run lint and checks" }
check = { program = "mvn", args = ["verify"], description = "Run checks" }
clean = { program = "mvn", args = ["clean"], description = "Remove build outputs" }
ci = { steps = ["fmt", "lint", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Maven starter assumes the exec and formatting plugins are configured",
};

const TEMPLATE_KOTLIN_GRADLE: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "./gradlew", args = ["build"], windows = { program = "gradlew.bat", args = ["build"] }, description = "Build the project" }
test = { program = "./gradlew", args = ["test"], windows = { program = "gradlew.bat", args = ["test"] }, description = "Run tests" }
run = { program = "./gradlew", args = ["run"], windows = { program = "gradlew.bat", args = ["run"] }, description = "Run the app" }
dev = { program = "./gradlew", args = ["run"], windows = { program = "gradlew.bat", args = ["run"] }, description = "Run the app" }
fmt = { program = "./gradlew", args = ["ktlintFormat"], windows = { program = "gradlew.bat", args = ["ktlintFormat"] }, description = "Format source files" }
lint = { program = "./gradlew", args = ["check"], windows = { program = "gradlew.bat", args = ["check"] }, description = "Run lint and checks" }
check = { program = "./gradlew", args = ["check"], windows = { program = "gradlew.bat", args = ["check"] }, description = "Run checks" }
clean = { program = "./gradlew", args = ["clean"], windows = { program = "gradlew.bat", args = ["clean"] }, description = "Remove build outputs" }
ci = { steps = ["fmt", "lint", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Kotlin Gradle starter assumes a wrapper script and configured plugins",
};

const TEMPLATE_DOTNET: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "dotnet", args = ["build"], description = "Build the project" }
test = { program = "dotnet", args = ["test"], description = "Run tests" }
run = { program = "dotnet", args = ["run"], description = "Run the app" }
dev = { program = "dotnet", args = ["watch", "run"], description = "Run the app in watch mode" }
fmt = { program = "dotnet", args = ["format"], description = "Format source files" }
lint = { program = "dotnet", args = ["build", "-warnaserror"], description = "Run lint and checks" }
check = { program = "dotnet", args = ["build"], description = "Run checks" }
clean = { program = "dotnet", args = ["clean"], description = "Remove build outputs" }
ci = { steps = ["fmt", "lint", "test", "build"], description = "Run the standard checks" }
"#,
    warning: ".NET starter assumes dotnet tooling and project conventions are configured",
};

const TEMPLATE_PHP_COMPOSER: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "composer", args = ["install"], description = "Install dependencies" }
test = { program = "composer", args = ["test"], description = "Run tests" }
run = { program = "composer", args = ["start"], description = "Run the app" }
dev = { program = "composer", args = ["start"], description = "Run the app" }
fmt = { program = "composer", args = ["fmt"], description = "Format source files" }
lint = { program = "composer", args = ["lint"], description = "Run lint checks" }
check = { program = "composer", args = ["validate"], description = "Validate project files" }
clean = { program = "composer", args = ["clean"], description = "Remove generated files" }
ci = { steps = ["fmt", "lint", "check", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Composer starter assumes scripts exist for test, fmt, lint, and start",
};

const TEMPLATE_RUBY_BUNDLER: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "bundle", args = ["exec", "rake", "build"], description = "Build the package" }
test = { program = "bundle", args = ["exec", "rspec"], description = "Run tests" }
run = { program = "bundle", args = ["exec", "ruby", "main.rb"], description = "Run the app" }
dev = { program = "bundle", args = ["exec", "ruby", "main.rb"], description = "Run the app" }
fmt = { program = "bundle", args = ["exec", "rubocop", "-A"], description = "Format source files" }
lint = { program = "bundle", args = ["exec", "rubocop"], description = "Run lint checks" }
check = { program = "bundle", args = ["exec", "rubocop"], description = "Run checks" }
clean = { program = "bundle", args = ["exec", "rake", "clean"], description = "Remove build outputs" }
ci = { steps = ["fmt", "lint", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Ruby Bundler starter assumes Rake, RSpec, and RuboCop are configured",
};

const TEMPLATE_RAILS: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "bin", args = ["rails", "assets:precompile"], description = "Precompile assets" }
test = { program = "bin", args = ["rails", "test"], description = "Run tests" }
run = { program = "bin", args = ["rails", "server"], description = "Run the app" }
dev = { program = "bin", args = ["rails", "server"], description = "Run the app" }
fmt = { program = "bundle", args = ["exec", "rubocop", "-A"], description = "Format source files" }
lint = { program = "bundle", args = ["exec", "rubocop"], description = "Run lint checks" }
check = { program = "bin", args = ["rails", "test"], description = "Run Rails checks" }
clean = { program = "bin", args = ["rails", "tmp:clear", "log:clear"], description = "Remove generated files" }
ci = { steps = ["fmt", "lint", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Rails starter assumes a Rails app with bin/rails and RuboCop configured",
};

const TEMPLATE_LARAVEL: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "composer", args = ["install"], description = "Install dependencies" }
test = { program = "php", args = ["artisan", "test"], description = "Run tests" }
run = { program = "php", args = ["artisan", "serve"], description = "Run the app" }
dev = { program = "php", args = ["artisan", "serve"], description = "Run the app" }
fmt = { program = "./vendor/bin/pint", args = [], description = "Format source files" }
lint = { program = "./vendor/bin/pint", args = ["--test"], description = "Run lint checks" }
check = { program = "php", args = ["artisan", "about"], description = "Validate the project" }
clean = { program = "php", args = ["artisan", "optimize:clear"], description = "Remove cached files" }
ci = { steps = ["fmt", "lint", "check", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Laravel starter assumes artisan and Pint are available",
};

const TEMPLATE_TERRAFORM: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "terraform", args = ["plan"], description = "Preview infrastructure changes" }
test = { program = "terraform", args = ["validate"], description = "Validate configuration" }
run = { program = "terraform", args = ["apply"], description = "Apply infrastructure changes" }
dev = { program = "terraform", args = ["plan"], description = "Preview infrastructure changes" }
fmt = { program = "terraform", args = ["fmt", "-recursive"], description = "Format configuration files" }
lint = { program = "terraform", args = ["validate"], description = "Validate configuration" }
check = { program = "terraform", args = ["validate"], description = "Run checks" }
clean = { program = "terraform", args = ["fmt", "-recursive"], description = "Normalize terraform files" }
ci = { steps = ["fmt", "lint", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Terraform starter assumes stateful operations are reviewed before apply",
};

const TEMPLATE_HELM: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "helm", args = ["package", "."], description = "Package the chart" }
test = { program = "helm", args = ["lint", "."], description = "Lint the chart" }
run = { program = "helm", args = ["template", "."], description = "Render the chart" }
dev = { program = "helm", args = ["template", "."], description = "Render the chart" }
fmt = { program = "helm", args = ["lint", "."], description = "Validate chart structure" }
lint = { program = "helm", args = ["lint", "."], description = "Lint the chart" }
check = { program = "helm", args = ["template", "."], description = "Render the chart" }
clean = { program = "sh", args = ["-c", "rm -f ./*.tgz"], description = "Remove packaged charts" }
ci = { steps = ["fmt", "lint", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Helm starter assumes chart packaging and linting are configured",
};

const TEMPLATE_DOCKER_COMPOSE: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "docker", args = ["compose", "build"], description = "Build service images" }
test = { program = "docker", args = ["compose", "config"], description = "Validate the compose file" }
run = { program = "docker", args = ["compose", "up"], description = "Start the stack" }
dev = { program = "docker", args = ["compose", "up", "--build"], description = "Start the stack and rebuild images" }
fmt = { program = "docker", args = ["compose", "config"], description = "Validate compose configuration" }
lint = { program = "docker", args = ["compose", "config"], description = "Validate compose configuration" }
check = { program = "docker", args = ["compose", "config"], description = "Validate compose configuration" }
clean = { program = "docker", args = ["compose", "down", "--volumes", "--remove-orphans"], description = "Stop and remove containers" }
ci = { steps = ["fmt", "lint", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Docker Compose starter assumes the compose file describes a runnable stack",
};

const TEMPLATE_PYTHON: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "python", args = ["-m", "build"], description = "Build the package" }
test = { program = "pytest", args = [], description = "Run tests" }
run = { program = "python", args = ["main.py"], description = "Run the app" }
dev = { program = "python", args = ["main.py"], description = "Run the local app" }
fmt = { program = "ruff", args = ["format", "."], description = "Format source files" }
lint = { program = "ruff", args = ["check", "."], description = "Run lint checks" }
typecheck = { program = "mypy", args = ["."], description = "Run static type checks" }
clean = { program = "python", args = ["-c", "import shutil; [shutil.rmtree(p, ignore_errors=True) for p in ('build', 'dist')]"], description = "Remove build outputs" }
ci = { steps = ["fmt", "lint", "typecheck", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Python starter assumes pytest and ruff are installed",
};

const TEMPLATE_POETRY: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "poetry", args = ["build"], description = "Build the package" }
test = { program = "poetry", args = ["run", "pytest"], description = "Run tests" }
run = { program = "poetry", args = ["run", "python", "main.py"], description = "Run the app" }
dev = { program = "poetry", args = ["run", "python", "main.py"], description = "Run the local app" }
fmt = { program = "poetry", args = ["run", "ruff", "format", "."], description = "Format source files" }
lint = { program = "poetry", args = ["run", "ruff", "check", "."], description = "Run lint checks" }
typecheck = { program = "poetry", args = ["run", "mypy", "."], description = "Run static type checks" }
clean = { program = "poetry", args = ["run", "python", "-c", "import shutil; [shutil.rmtree(p, ignore_errors=True) for p in ('build', 'dist')]"], description = "Remove build outputs" }
ci = { steps = ["fmt", "lint", "typecheck", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Poetry starter assumes poetry and project metadata are configured",
};

const TEMPLATE_UV: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "uv", args = ["build"], description = "Build the package" }
test = { program = "uv", args = ["run", "pytest"], description = "Run tests" }
run = { program = "uv", args = ["run", "python", "main.py"], description = "Run the app" }
dev = { program = "uv", args = ["run", "python", "main.py"], description = "Run the local app" }
fmt = { program = "uv", args = ["run", "ruff", "format", "."], description = "Format source files" }
lint = { program = "uv", args = ["run", "ruff", "check", "."], description = "Run lint checks" }
typecheck = { program = "uv", args = ["run", "mypy", "."], description = "Run static type checks" }
clean = { program = "uv", args = ["run", "python", "-c", "import shutil; [shutil.rmtree(p, ignore_errors=True) for p in ('build', 'dist')]"], description = "Remove build outputs" }
ci = { steps = ["fmt", "lint", "typecheck", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "uv starter assumes uv and Python tooling are installed",
};

const TEMPLATE_GO: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "go", args = ["build", "./..."], description = "Build all packages" }
test = { program = "go", args = ["test", "./..."], description = "Run tests" }
run = { program = "go", args = ["run", "."], description = "Run the app" }
dev = { program = "go", args = ["run", "."], description = "Run the app locally" }
fmt = { program = "gofmt", args = ["-w", "."], description = "Format source files" }
lint = { program = "go", args = ["test", "./..."], description = "Run the default validation checks" }
check = { program = "go", args = ["test", "./..."], description = "Run validation checks" }
clean = { program = "go", args = ["clean"], description = "Clean build cache" }
ci = { steps = ["fmt", "lint", "test", "build"], description = "Run the standard checks" }
"#,
    warning: "Go starter assumes gofmt and go tooling are installed",
};

const TEMPLATE_CARGO_WORKSPACE: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "cargo", args = ["build", "--workspace"], description = "Build workspace packages" }
test = { program = "cargo", args = ["test", "--workspace"], description = "Run workspace tests" }
run = { program = "cargo", args = ["run"], description = "Run the default workspace member" }
fmt = { program = "cargo", args = ["fmt", "--all"], description = "Format source files" }
docs = { program = "cargo", args = ["doc", "--workspace"], description = "Generate workspace documentation" }
lint = { program = "cargo", args = ["clippy", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"], description = "Run Clippy" }
check = { program = "cargo", args = ["check", "--workspace"], description = "Run workspace checks" }
clean = { program = "cargo", args = ["clean"], description = "Remove build artifacts" }
ci = { steps = ["fmt", "lint", "test"], description = "Run the standard checks" }
"#,
    warning: "Cargo workspace starter assumes a Rust workspace layout",
};

const TEMPLATE_CMAKE: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "cmake", args = ["-S", ".", "-B", "build"], description = "Configure the build" }
test = { program = "ctest", args = ["--test-dir", "build"], description = "Run tests" }
run = { program = "cmake", args = ["--build", "build"], description = "Build the default target" }
fmt = { program = "cmake-format", args = ["-i", "CMakeLists.txt"], description = "Format CMake files" }
lint = { program = "cmake", args = ["-S", ".", "-B", "build"], description = "Configure the build" }
check = { program = "cmake", args = ["-S", ".", "-B", "build"], description = "Validate the build configuration" }
clean = { program = "cmake", args = ["--build", "build", "--target", "clean"], description = "Remove build outputs" }
ci = { steps = ["fmt", "check", "test"], description = "Run the standard checks" }
"#,
    warning: "CMake starter uses a placeholder run target; replace it with your executable target",
};

const TEMPLATE_CMAKE_NINJA: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = { program = "cmake", args = ["-S", ".", "-B", "build", "-G", "Ninja"], description = "Configure the build" }
test = { program = "ctest", args = ["--test-dir", "build"], description = "Run tests" }
run = { program = "cmake", args = ["--build", "build"], description = "Build the default target" }
fmt = { program = "cmake-format", args = ["-i", "CMakeLists.txt"], description = "Format CMake files" }
lint = { program = "cmake", args = ["-S", ".", "-B", "build", "-G", "Ninja"], description = "Configure the build" }
check = { program = "cmake", args = ["-S", ".", "-B", "build", "-G", "Ninja"], description = "Validate the build configuration" }
clean = { program = "cmake", args = ["--build", "build", "--target", "clean"], description = "Remove build outputs" }
ci = { steps = ["fmt", "check", "test"], description = "Run the standard checks" }
"#,
    warning: "CMake Ninja starter assumes Ninja and CMake are installed",
};

const TEMPLATE_GENERIC: TemplateSpec = TemplateSpec {
    body: r#"[project]
name = "example"
root = "."

[commands]
build = "echo build"
test = "echo test"
run = "echo run"
fmt = "echo fmt"
clean = "echo clean"
ci = "echo ci"
"#,
    warning: "Generic starter is illustrative and should be customized",
};

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

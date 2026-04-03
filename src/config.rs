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
    pub env: Option<HashMap<String, String>>,
    pub commands: CommandsSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSection {
    pub name: Option<String>,
    pub root: Option<String>,
    pub shell: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommandsSection {
    pub build: Option<String>,
    pub test: Option<String>,
    pub run: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub name: Option<String>,
    pub root: PathBuf,
    pub env: HashMap<String, String>,
    pub shell: Option<String>,
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
            shell: None,
        });

        let root = match project.root {
            Some(root) => project_dir.join(root),
            None => project_dir.to_path_buf(),
        };

        if !root.exists() || !root.is_dir() {
            return Err(Error::InvalidProjectRoot { path: root });
        }

        Ok(Self {
            name: project.name,
            root,
            env: file.env.unwrap_or_default(),
            shell: project.shell,
            commands: file.commands,
        })
    }
}

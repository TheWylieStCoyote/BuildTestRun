use crate::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

pub fn discover_config(start: &Path) -> Result<PathBuf, Error> {
    discover_config_chain(start)?
        .last()
        .cloned()
        .ok_or_else(|| Error::ConfigNotFound {
            start: start.to_path_buf(),
        })
}

pub fn discover_config_chain(start: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut current = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    let mut found = Vec::new();

    loop {
        let candidate = current.join(crate::constants::CONFIG_FILE_NAME);
        if candidate.is_file() {
            found.push(candidate);
        }

        if !current.pop() {
            break;
        }
    }

    if found.is_empty() {
        Err(Error::ConfigNotFound {
            start: start.to_path_buf(),
        })
    } else {
        found.reverse();
        Ok(found)
    }
}

pub fn discover_project_paths(start: &Path) -> Result<Vec<PathBuf>, Error> {
    let root = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    if !root.is_dir() {
        return Err(Error::InvalidProjectRoot {
            path: root.to_path_buf(),
        });
    }

    let mut projects = Vec::new();
    collect_project_paths(&root, &mut projects)?;
    projects.sort();
    Ok(projects)
}

fn collect_project_paths(dir: &Path, projects: &mut Vec<PathBuf>) -> Result<(), Error> {
    for entry in fs::read_dir(dir).map_err(|source| Error::ConfigRead {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| Error::ConfigRead {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();

        if path.is_dir() {
            collect_project_paths(&path, projects)?;
        } else if path.file_name().and_then(|name| name.to_str())
            == Some(crate::constants::CONFIG_FILE_NAME)
        {
            projects.push(path);
        }
    }

    Ok(())
}

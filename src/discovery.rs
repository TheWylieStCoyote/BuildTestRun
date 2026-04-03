use crate::error::Error;
use std::path::{Path, PathBuf};

pub fn discover_config(start: &Path) -> Result<PathBuf, Error> {
    let mut current = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());

    loop {
        let candidate = current.join(".mbr.toml");
        if candidate.is_file() {
            return Ok(candidate);
        }

        if !current.pop() {
            break;
        }
    }

    Err(Error::ConfigNotFound {
        start: start.to_path_buf(),
    })
}

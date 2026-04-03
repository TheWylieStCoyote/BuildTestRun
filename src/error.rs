use crate::cli::Action;
use std::{io, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to resolve current working directory: {0}")]
    CurrentDir(#[from] io::Error),

    #[error("no .mbr.toml found starting from {start}")]
    ConfigNotFound { start: PathBuf },

    #[error("failed to read config {path}: {source}")]
    ConfigRead { path: PathBuf, source: io::Error },

    #[error("failed to parse config {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("missing `commands.{action}` in config")]
    MissingCommand { action: Action },

    #[error("invalid project root {path}")]
    InvalidProjectRoot { path: PathBuf },

    #[error("failed to execute command: {0}")]
    Execution(String),
}

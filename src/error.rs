use crate::cli::Action;
use std::{io, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to resolve current working directory: {0}")]
    CurrentDir(#[from] io::Error),

    #[error("no .btr.toml found starting from {start}")]
    ConfigNotFound { start: PathBuf },

    #[error("config already exists at {path}")]
    ConfigExists { path: PathBuf },

    #[error("failed to read config {path}: {source}")]
    ConfigRead { path: PathBuf, source: io::Error },

    #[error("failed to write config {path}: {source}")]
    ConfigWrite { path: PathBuf, source: io::Error },

    #[error("failed to read template {path}: {source}")]
    TemplateRead { path: PathBuf, source: io::Error },

    #[error("no template file found in {path}")]
    TemplateNotFound { path: PathBuf },

    #[error("generated init template is invalid: {source}")]
    InitTemplateParse { source: Box<toml::de::Error> },

    #[error("safe init template forbids shell command `{name}`")]
    UnsafeInitTemplate { name: String },

    #[error("failed to parse config {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },

    #[error("missing `commands.{action}` in config")]
    MissingCommand { action: Action },

    #[error("missing `[commands]` section in config")]
    MissingCommandGroup,

    #[error("unknown command `{name}`")]
    UnknownCommand { name: String },

    #[error("unknown base command `{base}` for `{name}`")]
    UnknownCommandBase { name: String, base: String },

    #[error("command inheritance cycle detected at `{name}`")]
    CommandInheritanceCycle { name: String },

    #[error("unknown profile `{name}`")]
    UnknownProfile { name: String },

    #[error("safe mode forbids shell command `{name}`")]
    UnsafeShellCommand { name: String },

    #[error("invalid project root {path}")]
    InvalidProjectRoot { path: PathBuf },

    #[error("failed to execute command: {0}")]
    Execution(String),

    #[error("failed to package project: {0}")]
    Package(String),
}

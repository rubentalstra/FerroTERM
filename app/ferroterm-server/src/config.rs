//! Configuration: the environment in, a [`Config`] out.

use std::path::PathBuf;

use crate::telemetry::{FILTER_ENV, FORMAT_ENV, FormatError, LogFormat};

/// The environment variable naming the socket address to listen on.
pub const LISTEN_ENV: &str = "FERROTERM_LISTEN";
/// The environment variable listing the artifact directories to serve,
/// separated by the platform's path separator (`:` on Unix).
pub const INDEX_ENV: &str = "FERROTERM_INDEX";
/// The environment variable naming the default display language (BCP 47).
pub const LANGUAGE_ENV: &str = "FERROTERM_DEFAULT_LANGUAGE";

/// What the server needs to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The socket address to bind.
    pub listen: String,
    /// The artifact directories to load, each one code system version.
    pub index: Vec<PathBuf>,
    /// The display language used when a request names none.
    pub default_language: String,
    /// The console log format.
    pub log_format: LogFormat,
    /// The `tracing` filter.
    pub log_filter: String,
}

/// A configuration value that does not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// `FERROTERM_LOG_FORMAT` names no format.
    #[error("{FORMAT_ENV}: {0}")]
    LogFormat(#[from] FormatError),
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: String::from("127.0.0.1:8080"),
            index: Vec::new(),
            default_language: String::from("en"),
            log_format: LogFormat::Auto,
            log_filter: String::from(crate::telemetry::DEFAULT_FILTER),
        }
    }
}

impl Config {
    /// Reads the configuration from the environment; an unset variable keeps
    /// the default.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when a variable holds a value that does not parse.
    pub fn from_env() -> Result<Self, ConfigError> {
        let defaults = Self::default();
        Ok(Self {
            listen: std::env::var(LISTEN_ENV).unwrap_or(defaults.listen),
            index: std::env::var_os(INDEX_ENV)
                .map(|value| std::env::split_paths(&value).collect())
                .unwrap_or_default(),
            default_language: std::env::var(LANGUAGE_ENV).unwrap_or(defaults.default_language),
            log_format: match std::env::var(FORMAT_ENV) {
                Ok(text) => text.parse()?,
                Err(_) => defaults.log_format,
            },
            log_filter: std::env::var(FILTER_ENV).unwrap_or(defaults.log_filter),
        })
    }
}

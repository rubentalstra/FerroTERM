//! Configuration: the environment in, a [`Config`] out.

use std::path::PathBuf;

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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: String::from("127.0.0.1:8080"),
            index: Vec::new(),
            default_language: String::from("en"),
        }
    }
}

impl Config {
    /// Reads the configuration from the environment; an unset variable keeps
    /// the default.
    #[must_use]
    pub fn from_env() -> Self {
        let defaults = Self::default();
        Self {
            listen: std::env::var(LISTEN_ENV).unwrap_or(defaults.listen),
            index: std::env::var_os(INDEX_ENV)
                .map(|value| std::env::split_paths(&value).collect())
                .unwrap_or_default(),
            default_language: std::env::var(LANGUAGE_ENV).unwrap_or(defaults.default_language),
        }
    }
}

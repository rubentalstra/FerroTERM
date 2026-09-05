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
/// The environment variable listing the `CodeSystem` resource directories.
///
/// Each is a FHIR package's `package/` directory or a directory of JSON files;
/// the platform's path separator separates them.
pub const CODESYSTEMS_ENV: &str = "FERROTERM_CODESYSTEMS";
/// The environment variable naming the database of persisted client resources.
///
/// The `CodeSystem`, `ValueSet`, and `ConceptMap` resources written through the
/// REST API live in this file, with the closure tables `$closure` maintains,
/// and are served again after a restart. A deployment that names none refuses
/// every write.
pub const RESOURCES_ENV: &str = "FERROTERM_RESOURCES";
/// The environment variable naming the authentication in front of the server.
///
/// Its value is codes of the FHIR `restful-security-service` value set
/// (`SMART-on-FHIR`, `OAuth`, `Basic`, `Certificates`, `Kerberos`, `NTLM`),
/// separated by commas.
pub const SECURITY_SERVICE_ENV: &str = "FERROTERM_SECURITY_SERVICE";
/// The environment variable naming the base URL clients reach this server at.
///
/// A server behind a reverse proxy answers on a URL it cannot see: the proxy
/// terminates TLS and forwards a plain request, so the address the process
/// bound is not the address a client used. The capability statements state
/// this value as `implementation.url`, per version, so a client that reads one
/// learns where to send the next request
/// (<https://hl7.org/fhir/R4B/capabilitystatement-definitions.html#CapabilityStatement.implementation.url>).
pub const BASE_URL_ENV: &str = "FERROTERM_BASE_URL";

/// The codes of the FHIR `restful-security-service` value set
/// (<http://hl7.org/fhir/ValueSet/restful-security-service>), each with its
/// display, in the code system's own order.
pub const SECURITY_SERVICES: [(&str, &str); 6] = [
    ("OAuth", "OAuth"),
    ("SMART-on-FHIR", "SMART-on-FHIR"),
    ("NTLM", "NTLM"),
    ("Basic", "Basic"),
    ("Kerberos", "Kerberos"),
    ("Certificates", "Certificates"),
];

/// The code system the security service codes come from.
pub const SECURITY_SERVICE_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/restful-security-service";

/// What the server needs to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The socket address to bind.
    pub listen: String,
    /// The artifact directories to load, each one code system version.
    pub index: Vec<PathBuf>,
    /// The directories of FHIR `CodeSystem` resources to load.
    pub code_systems: Vec<PathBuf>,
    /// The database of persisted client resources; `None` when the deployment
    /// persists none.
    pub resources: Option<PathBuf>,
    /// The display language used when a request names none.
    pub default_language: String,
    /// The console log format.
    pub log_format: LogFormat,
    /// The `tracing` filter.
    pub log_filter: String,
    /// The authentication in front of the server, as codes of the FHIR
    /// `restful-security-service` value set; empty when the deployment
    /// declares none.
    pub security_services: Vec<String>,
    /// The base URL clients reach this server at, without a version prefix
    /// and without a trailing slash; `None` when the deployment names none.
    pub base_url: Option<String>,
}

/// A configuration value that does not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// `FERROTERM_LOG_FORMAT` names no format.
    #[error("{FORMAT_ENV}: {0}")]
    LogFormat(#[from] FormatError),
    /// `FERROTERM_SECURITY_SERVICE` names a code the value set does not define.
    #[error(
        "{SECURITY_SERVICE_ENV}: `{0}` is not a code of http://hl7.org/fhir/ValueSet/restful-security-service"
    )]
    SecurityService(String),
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: String::from("127.0.0.1:8080"),
            index: Vec::new(),
            code_systems: Vec::new(),
            resources: None,
            default_language: String::from("en"),
            log_format: LogFormat::Auto,
            log_filter: String::from(crate::telemetry::DEFAULT_FILTER),
            security_services: Vec::new(),
            base_url: None,
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
            code_systems: std::env::var_os(CODESYSTEMS_ENV)
                .map(|value| std::env::split_paths(&value).collect())
                .unwrap_or_default(),
            resources: std::env::var_os(RESOURCES_ENV).map(PathBuf::from),
            default_language: std::env::var(LANGUAGE_ENV).unwrap_or(defaults.default_language),
            log_format: match std::env::var(FORMAT_ENV) {
                Ok(text) => text.parse()?,
                Err(_) => defaults.log_format,
            },
            log_filter: std::env::var(FILTER_ENV).unwrap_or(defaults.log_filter),
            security_services: security_services()?,
            base_url: std::env::var(BASE_URL_ENV)
                .ok()
                .map(|url| url.trim_end_matches('/').to_owned())
                .filter(|url| !url.is_empty()),
        })
    }
}

/// The security services `FERROTERM_SECURITY_SERVICE` names, each a code of
/// the FHIR `restful-security-service` value set.
///
/// # Errors
///
/// Returns [`ConfigError::SecurityService`] for a code the value set does not
/// define.
fn security_services() -> Result<Vec<String>, ConfigError> {
    let Ok(value) = std::env::var(SECURITY_SERVICE_ENV) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for name in value.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        let Some((code, _)) = SECURITY_SERVICES.iter().find(|(code, _)| *code == name) else {
            return Err(ConfigError::SecurityService(name.to_owned()));
        };
        out.push((*code).to_owned());
    }
    Ok(out)
}

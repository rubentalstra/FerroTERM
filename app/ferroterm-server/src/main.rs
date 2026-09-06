//! The `ferroterm` binary: configuration in, [`ferroterm_server::serve`] out.
//!
//! The order is the one that makes the console right: read the configuration,
//! print the banner when a person is reading, install the log subscriber, load
//! the artifacts (their summary is the first thing logged), bind, serve.
#![expect(
    clippy::print_stderr,
    reason = "a refused configuration is reported before any log subscriber exists"
)]

use std::io::IsTerminal;
use std::process::ExitCode;
use std::sync::Arc;

use ferroterm_server::config::{Config, INDEX_ENV, LISTEN_ENV, UI_ENV};
use ferroterm_server::state::AppState;
use ferroterm_server::telemetry::ResolvedFormat;
use ferroterm_server::{banner, telemetry};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("ferroterm: cannot start: {error}");
            return ExitCode::FAILURE;
        }
    };
    let stdout_is_terminal = std::io::stdout().is_terminal();
    if config.log_format.resolve(stdout_is_terminal) == ResolvedFormat::Pretty {
        banner::print();
    }
    if let Err(error) = telemetry::init(config.log_format, &config.log_filter, stdout_is_terminal) {
        eprintln!("ferroterm: cannot start: {error}");
        return ExitCode::FAILURE;
    }
    match run(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = format!("{error:#}"), "cannot start");
            ExitCode::FAILURE
        }
    }
}

async fn run(config: Config) -> anyhow::Result<()> {
    use anyhow::Context;

    let state = AppState::load(&config)
        .with_context(|| format!("loading the artifacts named by {INDEX_ENV}"))?;
    let summaries = state
        .summaries()
        .context("summarising the loaded code systems")?;
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        fhir_versions = "r4b",
        code_systems = summaries.len(),
        "ferroterm starting"
    );
    if summaries.is_empty() {
        tracing::warn!(
            "no code systems loaded: set {INDEX_ENV} to one or more artifact directories"
        );
    }
    for summary in &summaries {
        tracing::info!(
            id = summary.id,
            system = summary.url,
            version = summary.version,
            concepts = summary.concepts,
            languages = summary.languages.join(","),
            path = summary.path.as_ref().map(|p| p.display().to_string()),
            "serving code system"
        );
    }
    if config.viewer {
        if ferroterm_server::ui::BUNDLE.is_empty() {
            tracing::warn!(
                "{UI_ENV} is on and this binary carries no viewer bundle, so no /ui route is served"
            );
        } else {
            tracing::info!(base = ferroterm_server::ui::MOUNT, "serving the viewer");
        }
    }
    let listener = TcpListener::bind(&config.listen)
        .await
        .with_context(|| format!("binding {} (set {LISTEN_ENV} to change it)", config.listen))?;
    tracing::info!(listen = %config.listen, base = "/r4b", "listening");
    ferroterm_server::serve(listener, Arc::new(state))
        .await
        .context("serving HTTP")?;
    tracing::info!("ferroterm stopped");
    Ok(())
}

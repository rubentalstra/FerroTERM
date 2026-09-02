//! The `ferroterm` binary: configuration in, [`ferroterm_server::serve`] out.

use std::sync::Arc;

use anyhow::Context;
use ferroterm_server::config::{Config, INDEX_ENV, LISTEN_ENV};
use ferroterm_server::state::AppState;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::from_env();
    let state = AppState::load(&config)
        .with_context(|| format!("loading the artifacts named by {INDEX_ENV}"))?;
    for (id, url, version) in state.instances() {
        tracing::info!(id, url, version, "serving code system");
    }
    let listener = TcpListener::bind(&config.listen)
        .await
        .with_context(|| format!("binding {} (set {LISTEN_ENV} to change it)", config.listen))?;
    tracing::info!(listen = %config.listen, "ferroterm listening");

    ferroterm_server::serve(listener, Arc::new(state))
        .await
        .context("serving HTTP")
}

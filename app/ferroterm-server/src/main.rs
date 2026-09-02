//! The `ferroterm-server` binary: configuration in, [`ferroterm_server::serve`] out.

use anyhow::Context;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

/// The environment variable naming the socket address to listen on.
const LISTEN_ENV: &str = "FERROTERM_LISTEN";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let listen = std::env::var(LISTEN_ENV).unwrap_or_else(|_| String::from("127.0.0.1:8080"));
    let listener = TcpListener::bind(&listen)
        .await
        .with_context(|| format!("binding {listen} (set {LISTEN_ENV} to change it)"))?;
    tracing::info!(%listen, "ferroterm-server listening");

    ferroterm_server::serve(listener)
        .await
        .context("serving HTTP")
}

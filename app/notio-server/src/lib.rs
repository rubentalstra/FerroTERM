//! The Notio FHIR terminology server.
//!
//! The library holds the whole run path so integration tests can drive it:
//! [`router`] builds the `axum` application and [`serve`] runs it on a bound
//! listener. `main.rs` only parses configuration and calls in.
#![doc(test(attr(deny(warnings))))]

use axum::Router;
use axum::routing::get;
use http::StatusCode;
use tokio::net::TcpListener;

/// Builds the HTTP application.
///
/// Serves `GET /health`, which answers `200 OK` while the process is up. The
/// FHIR endpoints mount here as the terminology engine lands.
pub fn router() -> Router {
    Router::new().route("/health", get(health))
}

/// Serves [`router`] on an already-bound listener until the task is dropped.
///
/// # Errors
///
/// Returns the I/O error from accepting connections or serving them.
pub async fn serve(listener: TcpListener) -> std::io::Result<()> {
    axum::serve(listener, router()).await
}

async fn health() -> StatusCode {
    StatusCode::OK
}

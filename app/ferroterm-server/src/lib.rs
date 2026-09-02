//! The FerroTERM FHIR terminology server.
//!
//! The library holds the whole run path so integration tests can drive it:
//! [`router`] builds the `axum` application, [`serve`] runs it on a bound
//! listener until the process is asked to stop, and [`serve_until`] runs it
//! until any future completes. `main.rs` only parses configuration and calls in.
#![doc(test(attr(deny(warnings))))]

use std::future::Future;

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

/// Serves [`router`] on an already-bound listener until the process receives
/// `SIGTERM` or `SIGINT`, then finishes the connections in flight.
///
/// A container runtime stops a container with `SIGTERM` to PID 1 and kills it
/// after a grace period, so the server must answer the signal itself: there
/// is no shell and no init in the image to do it.
///
/// # Errors
///
/// Returns the I/O error from accepting connections or serving them.
pub async fn serve(listener: TcpListener) -> std::io::Result<()> {
    serve_until(listener, shutdown_signal()).await
}

/// Serves [`router`] on an already-bound listener until `shutdown` completes,
/// then finishes the connections in flight and returns.
///
/// # Errors
///
/// Returns the I/O error from accepting connections or serving them.
pub async fn serve_until<F>(listener: TcpListener, shutdown: F) -> std::io::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    axum::serve(listener, router())
        .with_graceful_shutdown(shutdown)
        .await
}

/// Completes when the process receives `SIGTERM` or `SIGINT`.
///
/// A failure to install a handler is logged and the future never completes,
/// so the server keeps serving and the runtime's kill after the grace period
/// remains the backstop.
pub async fn shutdown_signal() {
    let interrupt = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "cannot listen for SIGINT");
            std::future::pending::<()>().await;
        }
    };
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "cannot listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    tokio::select! {
        () = interrupt => tracing::info!("SIGINT received, shutting down"),
        () = terminate => tracing::info!("SIGTERM received, shutting down"),
    }
}

async fn health() -> StatusCode {
    StatusCode::OK
}

//! The FerroTERM FHIR terminology server.
//!
//! The library holds the whole run path so integration tests can drive it:
//! [`config::Config`] names the artifacts, [`state::AppState`] loads them into
//! a registry, [`router`] builds the `axum` application over that state, and
//! [`serve`] runs it on a bound listener until the process is asked to stop.
//! `main.rs` only reads the environment and calls in.
//!
//! Every served FHIR version has its own path prefix (`/r4`, `/r4b`, `/r5`), and within it
//! the resource and operation URLs the FHIR REST API defines
//! (<https://hl7.org/fhir/R4B/http.html>, <https://hl7.org/fhir/R4B/operations.html>).
#![doc(test(attr(deny(warnings))))]

pub mod banner;
pub mod config;
pub mod metrics;
pub mod outcome;
pub mod persistence;
pub mod r4;
pub mod r4b;
pub mod r5;
pub mod r6;
// The build script is the only caller: it `include!`s this source rather than
// linking the library it builds, so the module is compiled here for its tests.
#[cfg(test)]
mod release_date;
pub mod request_log;
pub mod scope;
pub mod state;
pub mod telemetry;
pub mod ui;
pub mod version;
pub mod wire;

use std::future::Future;
use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use http::StatusCode;
use tokio::net::TcpListener;

use crate::state::AppState;

/// Builds the HTTP application over `state`.
///
/// `GET /health` answers `200 OK` while the process is up; every FHIR route
/// lives under its version prefix. Any other path is an `OperationOutcome`
/// `not-found`.
pub fn router(state: Arc<AppState>) -> Router {
    router_with_bundle(state, ui::BUNDLE)
}

/// Builds the HTTP application over `state`, serving `bundle` as the viewer.
///
/// The viewer is mounted when the deployment asked for it and this binary
/// carries a bundle; otherwise `/` and `/ui` are the `OperationOutcome`
/// `not-found` every other unknown path answers. Taking the bundle as an
/// argument is what lets a test drive the viewer routes without the release
/// build's `dist/`.
pub fn router_with_bundle(state: Arc<AppState>, bundle: &'static [ui::Asset]) -> Router {
    let viewer = state.serves_viewer() && !bundle.is_empty();
    let mut app = Router::new()
        .route("/health", get(health))
        .nest("/r4", r4::router())
        .nest("/r4b", r4b::router())
        .nest("/r5", r5::router())
        .nest("/r6", r6::router())
        .route("/metrics", get(metrics_scrape));
    if viewer {
        app = app.route("/", get(ui::root));
    }
    let app = app
        .fallback(outcome::not_found)
        .layer(axum::middleware::from_fn_with_state(
            Arc::clone(&state),
            request_log::log,
        ));
    // NOTE: axum applies a layer only to the routes already added, so merging
    // the viewer here keeps its assets out of the request log and the metrics
    // (<https://docs.rs/axum/0.8/axum/struct.Router.html#method.layer>).
    let app = if viewer {
        app.merge(ui::router(bundle))
    } else {
        app
    };
    app.with_state(state)
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
pub async fn serve(listener: TcpListener, state: Arc<AppState>) -> std::io::Result<()> {
    serve_until(listener, state, shutdown_signal()).await
}

/// Serves [`router`] on an already-bound listener until `shutdown` completes,
/// then finishes the connections in flight and returns.
///
/// # Errors
///
/// Returns the I/O error from accepting connections or serving them.
pub async fn serve_until<F>(
    listener: TcpListener,
    state: Arc<AppState>,
    shutdown: F,
) -> std::io::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    axum::serve(listener, router(state))
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

/// `GET /metrics`: the Prometheus exposition of this server.
///
/// The endpoint is off the FHIR base path, so a scrape is never a terminology
/// request and no FHIR content negotiation applies. It answers the text
/// exposition format Prometheus reads
/// (<https://prometheus.io/docs/instrumenting/exposition_formats/>).
async fn metrics_scrape(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match state.metrics().exposition() {
        Ok(text) => (
            StatusCode::OK,
            [(
                http::header::CONTENT_TYPE,
                "application/openmetrics-text; version=1.0.0; charset=utf-8",
            )],
            text,
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "the metrics registry cannot be written");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

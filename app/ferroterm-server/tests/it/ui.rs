//! The viewer under `/ui`: what it answers, what it never reaches, and what
//! stays untouched outside it. No FHIR specification governs a terminology
//! server's user interface, so these are our own design (#403).

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use ferroterm_server::config::Config;
use ferroterm_server::state::AppState;
use ferroterm_server::ui::Asset;
use http::{Request, Response, StatusCode, header};
use tower::ServiceExt;

/// The document, one content-hashed asset, and one unhashed asset: the shapes
/// a Trunk bundle carries.
static BUNDLE: &[Asset] = &[
    Asset {
        path: "index.html",
        bytes: b"<!doctype html><title>FerroTERM viewer</title>",
    },
    Asset {
        path: "ferroterm-viewer-0123456789abcdef.js",
        bytes: b"export function boot() {}",
    },
    Asset {
        path: "robots.txt",
        bytes: b"User-agent: *\n",
    },
];

/// A server with no artifacts, serving `BUNDLE` when `viewer` is on.
fn app(viewer: bool) -> Router {
    ferroterm_server::router_with_bundle(state(viewer), BUNDLE)
}

fn state(viewer: bool) -> Arc<AppState> {
    let config = Config {
        viewer,
        ..Config::default()
    };
    Arc::new(AppState::load(&config).expect("a server without artifacts loads"))
}

async fn get(router: &Router, uri: &str) -> Response<Body> {
    let request = Request::get(uri).body(Body::empty()).expect("request");
    router.clone().oneshot(request).await.expect("response")
}

fn header_of(response: &Response<Body>, name: header::HeaderName) -> String {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

async fn body_of(response: Response<Body>) -> Vec<u8> {
    axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body")
        .to_vec()
}

#[tokio::test]
async fn the_bundle_answers_under_ui_with_its_media_type_and_cache_policy() {
    let app = app(true);

    let document = get(&app, "/ui/").await;
    assert_eq!(document.status(), StatusCode::OK);
    assert_eq!(
        header_of(&document, header::CONTENT_TYPE),
        "text/html; charset=utf-8"
    );
    assert_eq!(header_of(&document, header::CACHE_CONTROL), "no-cache");
    assert_eq!(
        header_of(&document, header::X_CONTENT_TYPE_OPTIONS),
        "nosniff"
    );
    assert_eq!(
        body_of(document).await,
        b"<!doctype html><title>FerroTERM viewer</title>"
    );

    let script = get(&app, "/ui/ferroterm-viewer-0123456789abcdef.js").await;
    assert_eq!(script.status(), StatusCode::OK);
    assert_eq!(
        header_of(&script, header::CONTENT_TYPE),
        "text/javascript; charset=utf-8"
    );
    assert_eq!(
        header_of(&script, header::CACHE_CONTROL),
        "public, max-age=31536000, immutable",
        "a content-hashed name is what makes the immutable claim true"
    );

    let robots = get(&app, "/ui/robots.txt").await;
    assert_eq!(
        header_of(&robots, header::CACHE_CONTROL),
        "no-cache",
        "an unhashed name is revalidated"
    );
}

#[tokio::test]
async fn a_client_side_route_deep_links_to_the_document() {
    let app = app(true);
    let deep = get(&app, "/ui/systems/http%3A%2F%2Fsnomed.info%2Fsct").await;
    assert_eq!(deep.status(), StatusCode::OK);
    assert_eq!(
        header_of(&deep, header::CONTENT_TYPE),
        "text/html; charset=utf-8"
    );
    assert_eq!(
        body_of(deep).await,
        b"<!doctype html><title>FerroTERM viewer</title>"
    );
}

#[tokio::test]
async fn the_mount_and_the_root_redirect_onto_the_viewer() {
    let app = app(true);
    for uri in ["/", "/ui"] {
        let response = get(&app, uri).await;
        assert_eq!(
            response.status(),
            StatusCode::TEMPORARY_REDIRECT,
            "{uri} redirects"
        );
        assert_eq!(header_of(&response, header::LOCATION), "/ui/", "{uri}");
    }
}

#[tokio::test]
async fn a_path_that_climbs_out_of_the_bundle_reaches_nothing() {
    let app = app(true);
    for uri in [
        "/ui/../../etc/passwd",
        "/ui/%2e%2e%2f%2e%2e%2fetc%2fpasswd",
        "/ui/....//....//etc/passwd",
        "/ui//etc/passwd",
    ] {
        let response = get(&app, uri).await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        assert_eq!(
            body_of(response).await,
            b"<!doctype html><title>FerroTERM viewer</title>",
            "{uri} answers the document, never a file"
        );
    }
}

#[tokio::test]
async fn an_unknown_path_outside_the_viewer_is_still_an_operation_outcome() {
    let app = app(true);
    for uri in ["/nope", "/r4b/Nonexistent", "/uixx"] {
        let response = get(&app, uri).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(
            header_of(&response, header::CONTENT_TYPE),
            "application/fhir+json; charset=utf-8",
            "{uri}"
        );
        let body: serde_json::Value =
            serde_json::from_slice(&body_of(response).await).expect("json");
        assert_eq!(body["resourceType"], "OperationOutcome", "{uri}");
        assert_eq!(body["issue"][0]["code"], "not-found", "{uri}");
    }
}

#[tokio::test]
async fn the_viewer_switched_off_removes_the_routes_and_restores_the_root() {
    let app = app(false);
    for uri in [
        "/",
        "/ui",
        "/ui/",
        "/ui/ferroterm-viewer-0123456789abcdef.js",
    ] {
        let response = get(&app, uri).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
        let body: serde_json::Value =
            serde_json::from_slice(&body_of(response).await).expect("json");
        assert_eq!(body["resourceType"], "OperationOutcome", "{uri}");
        assert_eq!(body["issue"][0]["code"], "not-found", "{uri}");
    }
}

#[tokio::test]
async fn a_binary_carrying_no_bundle_serves_no_viewer() {
    let app = ferroterm_server::router_with_bundle(state(true), &[]);
    let response = get(&app, "/ui/").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = serde_json::from_slice(&body_of(response).await).expect("json");
    assert_eq!(body["resourceType"], "OperationOutcome");
}

#[tokio::test]
async fn an_asset_is_neither_logged_nor_measured() {
    let state = state(true);
    let app = ferroterm_server::router_with_bundle(Arc::clone(&state), BUNDLE);
    for uri in ["/ui/", "/ui/ferroterm-viewer-0123456789abcdef.js"] {
        assert_eq!(get(&app, uri).await.status(), StatusCode::OK, "{uri}");
    }
    assert_eq!(
        get(&app, "/ui").await.status(),
        StatusCode::TEMPORARY_REDIRECT
    );
    let scrape = get(&app, "/metrics").await;
    assert_eq!(scrape.status(), StatusCode::OK);
    let exposition = String::from_utf8(body_of(scrape).await).expect("utf-8");
    assert!(
        !exposition.contains("/ui"),
        "the latency histograms describe the terminology operations only: {exposition}"
    );
}

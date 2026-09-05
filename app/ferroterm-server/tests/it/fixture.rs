//! A server state over the testkit's synthetic SNOMED edition, and request helpers.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use ferroterm_server::config::Config;
use ferroterm_server::state::AppState;
use http::{Request, Response, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

/// The edition in a temporary directory, loaded the way the binary loads it.
pub(crate) struct Server {
    _dir: Arc<tempfile::TempDir>,
    config: Config,
    pub(crate) state: Arc<AppState>,
}

impl Server {
    pub(crate) fn start() -> Self {
        Self::start_with(false, false)
    }

    /// The edition behind a proxy that reaches it at `base`.
    pub(crate) fn start_with_base_url(base: &str) -> Self {
        let mut server = Self::start_with(false, false);
        let config = Config {
            base_url: Some(base.to_owned()),
            ..server.config.clone()
        };
        server.state = Arc::new(AppState::load(&config).expect("loads"));
        server.config = config;
        server
    }

    /// The edition plus the testkit's `CodeSystem` and `ValueSet` resources.
    pub(crate) fn start_with_resources() -> Self {
        Self::start_with(true, false)
    }

    /// The edition, the testkit's resources, and a resource database, so the
    /// persisted-resource routes answer.
    pub(crate) fn start_persisting() -> Self {
        Self::start_with(true, true)
    }

    /// The edition, the testkit's `CodeSystem` resources, and the LOINC and
    /// `RxNorm` artifacts, so a test reaches a system of every loader beside
    /// the registry systems the binary always serves.
    pub(crate) fn start_with_every_loader() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let snomed = dir.path().join("snomed");
        let loinc = dir.path().join("loinc");
        let rxnorm = dir.path().join("rxnorm");
        let fhir = dir.path().join("fhir");
        for path in [&snomed, &loinc, &rxnorm, &fhir] {
            std::fs::create_dir_all(path).expect("creates");
        }
        ferroterm_testkit::snomed::write(&snomed).expect("writes the edition");
        ferroterm_testkit::loinc::write_artifact(&loinc).expect("builds loinc");
        ferroterm_testkit::rxnorm::write_artifact(&rxnorm).expect("builds rxnorm");
        ferroterm_testkit::fhir::write_code_systems(&fhir).expect("writes the resources");
        let config = Config {
            index: vec![snomed, loinc, rxnorm],
            code_systems: vec![fhir],
            ..Config::default()
        };
        let state = Arc::new(AppState::load(&config).expect("loads"));
        Self {
            _dir: Arc::new(dir),
            config,
            state,
        }
    }

    /// The `CodeSystem` instance id the server addresses `url` by.
    pub(crate) fn instance_id_of(&self, url: &str) -> String {
        self.state
            .instances()
            .find(|(_, served, _)| *served == url)
            .map_or_else(|| panic!("{url} is loaded"), |(id, _, _)| id.to_owned())
    }

    /// The same configuration loaded again, as a restart loads it.
    ///
    /// The running server is dropped first: `redb` holds the database file for
    /// as long as its handle lives
    /// (<https://docs.rs/redb/latest/redb/struct.Database.html>).
    pub(crate) fn restarted(self) -> Self {
        let Self {
            _dir: dir,
            config,
            state,
        } = self;
        drop(state);
        let state = Arc::new(AppState::load(&config).expect("reloads"));
        Self {
            _dir: dir,
            config,
            state,
        }
    }

    fn start_with(resources: bool, persists: bool) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        ferroterm_testkit::snomed::write(dir.path()).expect("writes the edition");
        let fhir = dir.path().join("fhir");
        std::fs::create_dir_all(&fhir).expect("creates");
        ferroterm_testkit::fhir::write_code_systems(&fhir).expect("writes the resources");
        let config = Config {
            index: vec![dir.path().to_path_buf()],
            code_systems: if resources { vec![fhir] } else { Vec::new() },
            resources: persists.then(|| dir.path().join("resources.redb")),
            ..Config::default()
        };
        let state = Arc::new(AppState::load(&config).expect("loads"));
        Self {
            _dir: Arc::new(dir),
            config,
            state,
        }
    }

    /// Any request, answered as the raw response so a test can read its headers.
    pub(crate) async fn send(&self, request: Request<Body>) -> Response<Body> {
        self.router().oneshot(request).await.expect("response")
    }

    /// A `PUT` of a FHIR JSON body, answered as the raw response.
    pub(crate) async fn put(&self, uri: &str, body: &Value) -> Response<Body> {
        let request = Request::put(uri)
            .header(http::header::CONTENT_TYPE, "application/fhir+json")
            .body(Body::from(body.to_string()))
            .expect("request");
        self.send(request).await
    }

    pub(crate) fn router(&self) -> Router {
        ferroterm_server::router(Arc::clone(&self.state))
    }

    /// The `CodeSystem` instance id of the synthetic edition.
    pub(crate) fn snomed_id(&self) -> String {
        self.state
            .instances()
            .next()
            .map(|(id, _, _)| id.to_owned())
            .expect("one instance")
    }

    pub(crate) async fn get(&self, uri: &str) -> (StatusCode, Value) {
        let request = Request::get(uri).body(Body::empty()).expect("request");
        json(self.router().oneshot(request).await.expect("response")).await
    }

    pub(crate) async fn get_with_header(
        &self,
        uri: &str,
        name: &str,
        value: &str,
    ) -> (StatusCode, Value) {
        let request = Request::get(uri)
            .header(name, value)
            .body(Body::empty())
            .expect("request");
        json(self.router().oneshot(request).await.expect("response")).await
    }

    pub(crate) async fn post_with_header(
        &self,
        uri: &str,
        body: &Value,
        name: &str,
        value: &str,
    ) -> (StatusCode, Value) {
        let request = Request::post(uri)
            .header(http::header::CONTENT_TYPE, "application/fhir+json")
            .header(name, value)
            .body(Body::from(body.to_string()))
            .expect("request");
        json(self.router().oneshot(request).await.expect("response")).await
    }

    pub(crate) async fn post(&self, uri: &str, body: &Value) -> (StatusCode, Value) {
        let request = Request::post(uri)
            .header(http::header::CONTENT_TYPE, "application/fhir+json")
            .body(Body::from(body.to_string()))
            .expect("request");
        json(self.router().oneshot(request).await.expect("response")).await
    }

    /// A `GET` with an optional `Accept`, answered as the status, the
    /// `Content-Type`, and the body text (for a response that is not FHIR JSON).
    pub(crate) async fn get_text(
        &self,
        uri: &str,
        accept: Option<&str>,
    ) -> (StatusCode, String, String) {
        let mut request = Request::get(uri);
        if let Some(accept) = accept {
            request = request.header(http::header::ACCEPT, accept);
        }
        let request = request.body(Body::empty()).expect("request");
        text(self.router().oneshot(request).await.expect("response")).await
    }

    /// A `POST` of `body` with `content_type` and an optional `Accept`, answered
    /// as the status, the `Content-Type`, and the body text.
    pub(crate) async fn post_text(
        &self,
        uri: &str,
        content_type: &str,
        body: &str,
        accept: Option<&str>,
    ) -> (StatusCode, String, String) {
        let mut request = Request::post(uri).header(http::header::CONTENT_TYPE, content_type);
        if let Some(accept) = accept {
            request = request.header(http::header::ACCEPT, accept);
        }
        let request = request.body(Body::from(body.to_owned())).expect("request");
        text(self.router().oneshot(request).await.expect("response")).await
    }

    pub(crate) async fn post_raw(
        &self,
        uri: &str,
        content_type: &str,
        body: &str,
    ) -> (StatusCode, Value) {
        let request = Request::post(uri)
            .header(http::header::CONTENT_TYPE, content_type)
            .body(Body::from(body.to_owned()))
            .expect("request");
        json(self.router().oneshot(request).await.expect("response")).await
    }
}

pub(crate) async fn json(response: Response<Body>) -> (StatusCode, Value) {
    let status = response.status();
    assert_eq!(
        response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/fhir+json; charset=utf-8"),
        "every FHIR response is FHIR JSON"
    );
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("body");
    let value: Value = serde_json::from_slice(&bytes).expect("json body");
    (status, value)
}

/// The status, `Content-Type`, and text of any response.
pub(crate) async fn text(response: Response<Body>) -> (StatusCode, String, String) {
    let status = response.status();
    let content_type = response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("body");
    (
        status,
        content_type,
        String::from_utf8(bytes.to_vec()).expect("utf-8"),
    )
}

/// The value of a named parameter of a `Parameters` body.
pub(crate) fn parameter<'a>(body: &'a Value, name: &str) -> Option<&'a Value> {
    body["parameter"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == name)
}

/// A `Parameters` body from (name, value) pairs.
pub(crate) fn parameters(pairs: &[(&str, Value)]) -> Value {
    let list: Vec<Value> = pairs
        .iter()
        .map(|(name, value)| {
            let mut object = serde_json::Map::new();
            object.insert(String::from("name"), Value::String((*name).to_owned()));
            if let Value::Object(v) = value {
                for (k, val) in v {
                    object.insert(k.clone(), val.clone());
                }
            }
            Value::Object(object)
        })
        .collect();
    serde_json::json!({"resourceType": "Parameters", "parameter": list})
}

/// A response header as text.
pub(crate) fn header(response: &Response<Body>, name: http::HeaderName) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

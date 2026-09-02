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
    _dir: tempfile::TempDir,
    pub(crate) state: Arc<AppState>,
}

impl Server {
    pub(crate) fn start() -> Self {
        Self::start_with(false)
    }

    /// The edition plus the testkit's `CodeSystem` and `ValueSet` resources.
    pub(crate) fn start_with_resources() -> Self {
        Self::start_with(true)
    }

    fn start_with(resources: bool) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        ferroterm_testkit::snomed::write(dir.path()).expect("writes the edition");
        let fhir = dir.path().join("fhir");
        std::fs::create_dir_all(&fhir).expect("creates");
        ferroterm_testkit::fhir::write_code_systems(&fhir).expect("writes the resources");
        let config = Config {
            index: vec![dir.path().to_path_buf()],
            code_systems: if resources { vec![fhir] } else { Vec::new() },
            ..Config::default()
        };
        let state = Arc::new(AppState::load(&config).expect("loads"));
        Self { _dir: dir, state }
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

    pub(crate) async fn post(&self, uri: &str, body: &Value) -> (StatusCode, Value) {
        let request = Request::post(uri)
            .header(http::header::CONTENT_TYPE, "application/fhir+json")
            .body(Body::from(body.to_string()))
            .expect("request");
        json(self.router().oneshot(request).await.expect("response")).await
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

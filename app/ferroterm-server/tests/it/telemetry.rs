//! The JSON log format carries the startup and request fields a pipeline reads.

use std::io::Write;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use ferroterm_server::telemetry::{ResolvedFormat, subscriber};
use http::Request;
use serde_json::Value;
use tower::ServiceExt;
use tracing_subscriber::fmt::MakeWriter;

use crate::fixture::Server;

/// A writer into a shared buffer, for reading the lines back.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = CaptureWriter;

    fn make_writer(&'a self) -> CaptureWriter {
        CaptureWriter(Arc::clone(&self.0))
    }
}

impl Capture {
    fn lines(&self) -> Vec<Value> {
        let bytes = self.0.lock().expect("lock").clone();
        String::from_utf8(bytes)
            .expect("utf-8")
            .lines()
            .map(|line| serde_json::from_str(line).unwrap_or_else(|_| panic!("not JSON: {line}")))
            .collect()
    }
}

#[tokio::test]
async fn json_lines_carry_the_startup_and_request_fields() {
    let capture = Capture::default();
    let subscriber = subscriber(
        ResolvedFormat::Json,
        "info,hyper=warn",
        false,
        capture.clone(),
    );
    let _guard = tracing::subscriber::set_default(subscriber);

    tracing::info!(
        listen = "127.0.0.1:8080",
        code_systems = 1_u64,
        "ferroterm starting"
    );
    let server = Server::start();
    let request =
        Request::get("/r4b/CodeSystem/$lookup?system=http://snomed.info/sct&code=1&display=x")
            .body(Body::empty())
            .expect("request");
    let response = server.router().oneshot(request).await.expect("response");
    assert!(response.status().is_client_error());

    let lines = capture.lines();
    let start = lines
        .iter()
        .find(|l| l["message"] == "ferroterm starting")
        .expect("startup line");
    assert_eq!(start["level"], "INFO");
    assert_eq!(start["listen"], "127.0.0.1:8080");
    assert_eq!(start["code_systems"], 1);
    assert!(start["timestamp"].as_str().is_some());
    let request_line = lines
        .iter()
        .find(|l| l["message"] == "request")
        .expect("request line");
    assert_eq!(request_line["level"], "WARN", "a client error logs at warn");
    assert_eq!(request_line["method"], "GET");
    assert_eq!(request_line["route"], "/r4b/CodeSystem/$lookup");
    assert_eq!(request_line["status"], 400);
    assert!(request_line["latency_ms"].as_f64().is_some());
    assert_eq!(
        request_line["named"],
        "system=http://snomed.info/sct code=1"
    );
    assert!(
        request_line.get("display").is_none(),
        "free-text parameters are not logged"
    );
}

#[tokio::test]
async fn pretty_lines_stay_on_one_line() {
    let capture = Capture::default();
    let subscriber = subscriber(ResolvedFormat::Pretty, "info", false, capture.clone());
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::info!(value = "two\nlines", "one event");
    let bytes = capture.0.lock().expect("lock").clone();
    let text = String::from_utf8(bytes).expect("utf-8");
    assert_eq!(text.lines().count(), 1, "the interior line feed is escaped");
    assert!(text.contains("two\\nlines"));
    assert!(!text.contains("\u{1b}["), "no colour when ansi is off");
}

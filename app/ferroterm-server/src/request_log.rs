//! One log line per HTTP request: method, route, status, latency, and the
//! system and code the request named. Bodies are never logged.

use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;

/// The query parameters worth a log field: they name a system or a code and
/// never carry free text.
const LOGGED_QUERY: [&str; 6] = ["system", "url", "version", "code", "codeA", "codeB"];

/// Logs the request after it completes.
pub async fn log(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let route = request.extensions().get::<MatchedPath>().map_or_else(
        || request.uri().path().to_owned(),
        |m| m.as_str().to_owned(),
    );
    let named = named_parameters(request.uri().query().unwrap_or(""));
    let started = Instant::now();
    let response = next.run(request).await;
    let status = response.status();
    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
    let (status_code, method, route, named) = (
        status.as_u16(),
        method.as_str(),
        route.as_str(),
        named.as_str(),
    );
    if status.is_server_error() {
        tracing::error!(
            method,
            route,
            status = status_code,
            latency_ms,
            named,
            "request"
        );
    } else if status.is_client_error() {
        tracing::warn!(
            method,
            route,
            status = status_code,
            latency_ms,
            named,
            "request"
        );
    } else {
        tracing::info!(
            method,
            route,
            status = status_code,
            latency_ms,
            named,
            "request"
        );
    }
    response
}

/// `key=value` pairs of the logged query parameters, space-separated, in
/// query order; the value is taken verbatim up to 64 characters.
fn named_parameters(query: &str) -> String {
    let mut out = String::new();
    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        if LOGGED_QUERY.contains(&key) {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(key);
            out.push('=');
            out.extend(value.chars().take(64));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::named_parameters;

    #[test]
    fn only_system_and_code_parameters_are_logged() {
        assert_eq!(
            named_parameters("system=http://snomed.info/sct&code=1&display=Secret%20text&codeA=2"),
            "system=http://snomed.info/sct code=1 codeA=2"
        );
        assert_eq!(named_parameters(""), "");
    }
}

//! The Prometheus metrics a deployment scrapes, and the registry behind them.
//!
//! No FHIR specification governs this: our own design, kept off the FHIR base
//! path so a scrape is never a terminology request. The exposition follows the
//! `OpenMetrics` text format the Prometheus client writes
//! (<https://prometheus.io/docs/instrumenting/exposition_formats/>).

use prometheus_client::encoding::text::encode;
use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::{Histogram, exponential_buckets};
use prometheus_client::registry::Registry;

/// The labels of one request: the matched route rather than the URI, so a
/// scrape has one series per operation instead of one per code.
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct Request {
    /// The HTTP method.
    pub method: Method,
    /// The matched route, `/r4b/CodeSystem/$lookup` rather than the URI.
    pub route: String,
    /// The status the server answered.
    pub status: u16,
}

/// The HTTP methods the server routes.
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelValue)]
pub enum Method {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
    /// `PUT`.
    Put,
    /// `DELETE`.
    Delete,
    /// Anything else, so an unrouted method still counts.
    Other,
}

impl From<&http::Method> for Method {
    fn from(method: &http::Method) -> Self {
        match *method {
            http::Method::GET => Self::Get,
            http::Method::POST => Self::Post,
            http::Method::PUT => Self::Put,
            http::Method::DELETE => Self::Delete,
            _ => Self::Other,
        }
    }
}

/// The labels of one loaded code system version.
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct System {
    /// The system URI.
    pub system: String,
    /// The version served.
    pub version: String,
}

/// The metrics of one running server.
#[derive(Debug)]
pub struct Metrics {
    registry: Registry,
    requests: Family<Request, Counter>,
    durations: Family<Request, Histogram>,
    systems: Family<System, Gauge>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    /// The registry with every metric this server exposes, empty of samples.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = <Registry>::with_prefix("ferroterm");
        let requests = Family::<Request, Counter>::default();
        registry.register(
            "http_requests",
            "The requests answered, by route and status",
            requests.clone(),
        );
        // A point read is held to a millisecond and an expansion page to ten
        // (`docs/architecture.md`), so the buckets start below both and reach a
        // slow request without a long tail of empty ones.
        let durations = Family::<Request, Histogram>::new_with_constructor(|| {
            Histogram::new(exponential_buckets(0.000_5, 2.0, 12))
        });
        registry.register_with_unit(
            "http_request_duration",
            "How long the server took to answer, by route and status",
            prometheus_client::registry::Unit::Seconds,
            durations.clone(),
        );
        let systems = Family::<System, Gauge>::default();
        registry.register(
            "code_system_loaded",
            "One per code system version the server loaded",
            systems.clone(),
        );
        Self {
            registry,
            requests,
            durations,
            systems,
        }
    }

    /// Records one answered request.
    pub fn record(&self, request: &Request, seconds: f64) {
        self.requests.get_or_create(request).inc();
        self.durations.get_or_create(request).observe(seconds);
    }

    /// Declares a code system version the server serves.
    pub fn loaded(&self, system: &str, version: &str) {
        self.systems
            .get_or_create(&System {
                system: system.to_owned(),
                version: version.to_owned(),
            })
            .set(1);
    }

    /// The exposition text a scrape reads.
    ///
    /// # Errors
    ///
    /// Returns the formatting error when the registry cannot be written, which
    /// means a metric name or label is not encodable.
    pub fn exposition(&self) -> Result<String, std::fmt::Error> {
        let mut out = String::new();
        encode(&mut out, &self.registry)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::{Method, Metrics, Request};

    #[test]
    fn the_exposition_carries_the_recorded_series() {
        let metrics = Metrics::new();
        metrics.loaded(
            "http://snomed.info/sct",
            "http://snomed.info/sct/1/version/2",
        );
        metrics.record(
            &Request {
                method: Method::Get,
                route: String::from("/r4b/CodeSystem/$lookup"),
                status: 200,
            },
            0.001_5,
        );
        let text = metrics.exposition().expect("encodes");
        assert!(
            text.contains(
                "ferroterm_http_requests_total{method=\"Get\",route=\"/r4b/CodeSystem/$lookup\",status=\"200\"} 1"
            ),
            "{text}"
        );
        assert!(
            text.contains("ferroterm_http_request_duration_seconds_count{"),
            "{text}"
        );
        assert!(
            text.contains("ferroterm_code_system_loaded{system=\"http://snomed.info/sct\""),
            "{text}"
        );
    }
}

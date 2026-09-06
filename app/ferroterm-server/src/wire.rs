//! The wire format of a request and a response: FHIR JSON or FHIR XML.
//!
//! A client names the response format with `_format` in the query, else with
//! `Accept`; a request body's format is its `Content-Type`
//! (<https://hl7.org/fhir/R4B/http.html#mime-type>). JSON is the default. XML
//! goes through the generated codec's schema of the served version
//! (<https://hl7.org/fhir/R4B/xml.html>).

use axum::body::Body;
use axum::response::{IntoResponse, Response};
use fhir_types::codec::Object;
use fhir_types::xml::Schemas;
use http::header::CONTENT_TYPE;
use http::{HeaderMap, StatusCode};

use crate::outcome::Failure;

/// The FHIR JSON media type of a response.
pub const FHIR_JSON: &str = "application/fhir+json; charset=utf-8";
/// The FHIR XML media type of a response.
pub const FHIR_XML: &str = "application/fhir+xml; charset=utf-8";
/// The query parameter naming the format.
pub const FORMAT: &str = "_format";

/// A wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Wire {
    /// FHIR JSON.
    #[default]
    Json,
    /// FHIR XML.
    Xml,
}

impl Wire {
    /// The format a media type or `_format` value names; `None` for one the
    /// server does not speak. The wildcards and the plain JSON types count as
    /// JSON (<https://hl7.org/fhir/R4B/http.html#mime-type>).
    #[must_use]
    pub fn of_media(media: &str) -> Option<Self> {
        let media = media.split(';').next().unwrap_or("").trim();
        match media.to_ascii_lowercase().as_str() {
            "json" | "application/json" | "application/fhir+json" | "*/*" | "application/*" => {
                Some(Self::Json)
            }
            "xml" | "text/xml" | "application/xml" | "application/fhir+xml" => Some(Self::Xml),
            _ => None,
        }
    }

    /// The response format a request asks for: `_format` in the query first,
    /// then the first acceptable media type of `Accept`; JSON when neither says.
    ///
    /// # Errors
    ///
    /// Returns a `406` failure for a `_format` the server does not speak.
    pub fn negotiate(query: &[(String, String)], headers: &HeaderMap) -> Result<Self, Failure> {
        if let Some((_, value)) = query.iter().find(|(name, _)| name == FORMAT) {
            return Self::of_media(value).ok_or_else(|| {
                Failure::new(
                    StatusCode::NOT_ACCEPTABLE,
                    "not-supported",
                    format!("`_format={value}` is not FHIR JSON or FHIR XML"),
                )
            });
        }
        let accept = headers
            .get(http::header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        Ok(accept
            .split(',')
            .find_map(Self::of_media)
            .unwrap_or_default())
    }

    /// The format of a request body, by `Content-Type`.
    ///
    /// # Errors
    ///
    /// Returns a `415` failure for a media type other than FHIR JSON or XML.
    pub fn of_body(headers: &HeaderMap) -> Result<Self, Failure> {
        let content_type = headers
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let media = content_type.split(';').next().unwrap_or("").trim();
        match Self::of_media(media) {
            Some(wire) if media != "*/*" && media != "application/*" => Ok(wire),
            _ => Err(Failure::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "not-supported",
                format!("content type `{media}` is not FHIR JSON or FHIR XML"),
            )),
        }
    }

    /// The `Content-Type` of a response in this format.
    #[must_use]
    pub const fn content_type(self) -> &'static str {
        match self {
            Self::Json => FHIR_JSON,
            Self::Xml => FHIR_XML,
        }
    }

    /// A response carrying `object`, a resource in the JSON object model, in
    /// this format; `schemas` is the served version's XML schema.
    #[must_use]
    pub fn response(self, status: StatusCode, object: &Object, schemas: &Schemas) -> Response {
        let body = match self {
            // NOTE: the map serializes by reference
            // (<https://docs.rs/serde_json/1/serde_json/fn.to_vec.html>); wrapping it
            // in a `Value` would deep-copy the whole resource to serialize it.
            Self::Json => match serde_json::to_vec(object) {
                Ok(json) => json,
                Err(_) => return status.into_response(),
            },
            Self::Xml => match fhir_types::xml::to_xml(schemas, object) {
                Ok(xml) => xml.into_bytes(),
                // NOTE: a resource the server built always has an XML form; if the
                // codec ever refuses, the status alone still tells the client.
                Err(_) => return status.into_response(),
            },
        };
        self.body(status, body)
    }

    /// A response carrying `resource`, a typed FHIR resource, in this format;
    /// `schemas` is the served version's XML schema.
    ///
    /// JSON writes the resource straight to bytes through the generated
    /// `Serialize`, with no intermediate document. XML converts through the
    /// object model its schema reads (<https://hl7.org/fhir/R4B/xml.html>).
    ///
    /// # Errors
    ///
    /// Returns a `500` failure when the resource has no form in this wire.
    pub fn resource<R: fhir_types::codec::Json + serde::Serialize>(
        self,
        status: StatusCode,
        resource: &R,
        schemas: &Schemas,
    ) -> Result<Response, Failure> {
        let body = match self {
            Self::Json => serde_json::to_vec(resource).map_err(|e| encoding(&e))?,
            Self::Xml => {
                let object = resource.to_json().map_err(|e| encoding(&e))?;
                fhir_types::xml::to_xml(schemas, &object)
                    .map_err(|e| encoding(&e))?
                    .into_bytes()
            }
        };
        Ok(self.body(status, body))
    }

    /// The response of `body` with this format's `Content-Type`.
    fn body(self, status: StatusCode, body: Vec<u8>) -> Response {
        Response::builder()
            .status(status)
            .header(CONTENT_TYPE, self.content_type())
            .body(Body::from(body))
            .unwrap_or_else(|_| status.into_response())
    }
}

/// The `500` a resource with no wire form answers with.
fn encoding(error: &dyn std::fmt::Display) -> Failure {
    Failure::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "exception",
        format!("cannot encode the response: {error}"),
    )
}

/// The query without its `_format`, for the operation's own parameters.
#[must_use]
pub fn without_format(query: &[(String, String)]) -> Vec<(String, String)> {
    query
        .iter()
        .filter(|(name, _)| name != FORMAT)
        .cloned()
        .collect()
}

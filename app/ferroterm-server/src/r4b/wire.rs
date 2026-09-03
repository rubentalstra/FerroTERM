//! From the wire to a `Parameters` and back
//! (<https://hl7.org/fhir/R4B/operations.html#3.2.0.6>).
//!
//! `GET` carries the primitive parameters as query parameters, a repeated
//! parameter as a repeated name; `POST` carries a `Parameters` resource as
//! FHIR JSON. A parameter the operation does not declare, or a complex one on
//! `GET`, is refused.

use axum::body::Bytes;
use axum::response::Response;
use ferroterm_fhir::codec::{Json, Path, expect_object};
use ferroterm_fhir::operation::{Operation, ParameterUse, ParametersError};
use ferroterm_fhir::r4b::parameters::{Parameters, ParametersParameter, ParametersParameterValue};
use http::{HeaderMap, StatusCode};

use crate::outcome::{Failure, fhir_json};

/// Builds the `Parameters` of a `GET` invocation from its query parameters.
///
/// # Errors
///
/// Returns a `400` failure for an undeclared parameter, a complex parameter
/// (which needs `POST`), or a value that is not of the parameter's type.
pub fn parameters_from_query(
    operation: &Operation,
    query: &[(String, String)],
) -> Result<Parameters, Failure> {
    let mut parameters = Vec::with_capacity(query.len());
    for (name, text) in query {
        let declared = operation.parameter(ParameterUse::In, name).ok_or_else(|| {
            Failure::new(
                StatusCode::BAD_REQUEST,
                "invalid",
                format!(
                    "{}/${} does not declare a parameter `{name}`",
                    operation.resource, operation.code
                ),
            )
        })?;
        let Some(type_code) = declared.type_code else {
            return Err(complex(operation, name));
        };
        let value = primitive(type_code, text)
            .ok_or_else(|| complex(operation, name))?
            .map_err(|reason| {
                Failure::new(
                    StatusCode::BAD_REQUEST,
                    "value",
                    format!("parameter `{name}` is not a valid {type_code}: {reason}"),
                )
            })?;
        parameters.push(ParametersParameter {
            name: name.as_str().into(),
            value: Some(value),
            ..Default::default()
        });
    }
    Ok(Parameters {
        parameter: parameters,
        ..Default::default()
    })
}

/// Adds a `displayLanguage` parameter from the `Accept-Language` header.
///
/// Only when the operation declares the parameter and the request named none:
/// the terminology ecosystem IG requires both routes
/// (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/languages.html>), and
/// the parameter wins when both are given.
pub fn apply_accept_language(
    operation: &Operation,
    headers: &HeaderMap,
    parameters: &mut Parameters,
) {
    if operation
        .parameter(ParameterUse::In, "displayLanguage")
        .is_none()
        || parameters
            .parameter
            .iter()
            .any(|p| p.name.value.as_deref() == Some("displayLanguage"))
    {
        return;
    }
    let Some(language) = headers
        .get(http::header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        return;
    };
    parameters.parameter.push(ParametersParameter {
        name: "displayLanguage".into(),
        value: Some(ParametersParameterValue::Code(language.into())),
        ..Default::default()
    });
}

fn complex(operation: &Operation, name: &str) -> Failure {
    Failure::new(
        StatusCode::BAD_REQUEST,
        "not-supported",
        format!(
            "parameter `{name}` of {}/${} is not a primitive; send it in a Parameters resource with POST",
            operation.resource, operation.code
        ),
    )
}

/// The typed value of a primitive parameter given as text; `None` when the
/// type is not a primitive the query form carries.
fn primitive(type_code: &str, text: &str) -> Option<Result<ParametersParameterValue, String>> {
    Some(match type_code {
        "code" => Ok(ParametersParameterValue::Code(text.into())),
        "uri" => Ok(ParametersParameterValue::Uri(text.into())),
        "url" => Ok(ParametersParameterValue::Url(text.into())),
        "canonical" => Ok(ParametersParameterValue::Canonical(text.into())),
        "string" => Ok(ParametersParameterValue::String(text.into())),
        "dateTime" => Ok(ParametersParameterValue::DateTime(text.into())),
        "date" => Ok(ParametersParameterValue::Date(text.into())),
        "decimal" => Ok(ParametersParameterValue::Decimal(text.into())),
        "boolean" => match text {
            "true" => Ok(ParametersParameterValue::Boolean(true.into())),
            "false" => Ok(ParametersParameterValue::Boolean(false.into())),
            other => Err(format!("`{other}` is not `true` or `false`")),
        },
        "integer" => text
            .parse::<i32>()
            .map(|i| ParametersParameterValue::Integer(i.into()))
            .map_err(|e| e.to_string()),
        _ => return None,
    })
}

/// Reads the `Parameters` resource of a `POST` invocation.
///
/// # Errors
///
/// Returns `415` for a media type other than FHIR JSON and `400` for a body
/// that is not a `Parameters` resource.
pub fn parameters_from_body(headers: &HeaderMap, body: &Bytes) -> Result<Parameters, Failure> {
    let content_type = headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let media = content_type.split(';').next().unwrap_or("").trim();
    if media != "application/fhir+json" && media != "application/json" {
        return Err(Failure::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "not-supported",
            format!("content type `{media}` is not FHIR JSON"),
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(body).map_err(|e| {
        Failure::new(
            StatusCode::BAD_REQUEST,
            "structure",
            format!("the body is not JSON: {e}"),
        )
    })?;
    let mut path = Path::root("Parameters");
    let object = expect_object(&value, &path)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "structure", e.to_string()))?;
    Parameters::from_json(object, &mut path)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "structure", e.to_string()))
}

/// The `400` failure for a `Parameters` that does not fit the operation.
#[must_use]
pub fn parameters_failure(error: &ParametersError) -> Failure {
    let code = match error {
        ParametersError::Missing { .. } => "required",
        _ => "invalid",
    };
    Failure::new(StatusCode::BAD_REQUEST, code, error.to_string())
}

/// A `200` response carrying `parameters` as FHIR JSON.
///
/// # Errors
///
/// Returns a `500` failure when the resource cannot be encoded.
pub fn respond(parameters: &Parameters) -> Result<Response, Failure> {
    respond_resource(parameters)
}

/// A `200` response carrying any FHIR resource as JSON.
///
/// An operation whose only output is a resource named `return` answers with
/// that resource itself (<https://hl7.org/fhir/R4B/operations.html#response>).
///
/// # Errors
///
/// Returns a `500` failure when the resource cannot be encoded.
pub fn respond_resource<R: Json>(resource: &R) -> Result<Response, Failure> {
    let object = resource.to_json().map_err(|e| {
        Failure::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "exception",
            format!("cannot encode the response: {e}"),
        )
    })?;
    Ok(fhir_json(
        StatusCode::OK,
        &serde_json::Value::Object(object),
    ))
}

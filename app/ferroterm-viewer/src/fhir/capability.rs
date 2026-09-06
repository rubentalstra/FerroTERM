//! The part of a `CapabilityStatement` the shell renders.

use serde::Deserialize;

/// What `GET [base]/metadata` tells the viewer about a served root.
///
/// The FHIR RESTful API defines `metadata` as the capabilities interaction
/// (<https://hl7.org/fhir/R4B/http.html#capabilities>), and the viewer reads
/// only the three facts the shell shows. Every field is optional here so a
/// server that omits one still renders.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct CapabilityStatement {
    /// `CapabilityStatement.fhirVersion`, the release this root speaks.
    #[serde(rename = "fhirVersion")]
    pub(crate) fhir_version: Option<String>,
    /// `CapabilityStatement.software`, which names the running server.
    pub(crate) software: Option<Software>,
}

/// `CapabilityStatement.software`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct Software {
    /// The software's own name.
    pub(crate) name: Option<String>,
    /// The version of the running software.
    pub(crate) version: Option<String>,
}

impl CapabilityStatement {
    /// The one line the shell shows about a root, or `None` when the server
    /// declared nothing the viewer reads.
    pub(crate) fn summary(&self) -> Option<String> {
        let software = self.software.as_ref().and_then(|software| {
            match (software.name.as_deref(), software.version.as_deref()) {
                (Some(name), Some(version)) => Some(format!("{name} {version}")),
                (Some(name), None) => Some(name.to_owned()),
                (None, Some(version)) => Some(version.to_owned()),
                (None, None) => None,
            }
        });
        match (self.fhir_version.as_deref(), software) {
            (Some(fhir), Some(software)) => Some(format!("FHIR {fhir}, {software}")),
            (Some(fhir), None) => Some(format!("FHIR {fhir}")),
            (None, software) => software,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> CapabilityStatement {
        serde_json::from_str(json).expect("the fixture is valid JSON")
    }

    #[test]
    fn the_summary_names_the_release_and_the_software() {
        let statement = parse(
            r#"{"resourceType":"CapabilityStatement","fhirVersion":"4.3.0",
                "software":{"name":"FerroTERM","version":"0.1.0"}}"#,
        );
        assert_eq!(
            statement.summary(),
            Some("FHIR 4.3.0, FerroTERM 0.1.0".to_owned())
        );
    }

    #[test]
    fn a_statement_without_software_still_names_its_release() {
        let statement = parse(r#"{"fhirVersion":"5.0.0"}"#);
        assert_eq!(statement.summary(), Some("FHIR 5.0.0".to_owned()));
    }

    #[test]
    fn a_statement_that_declares_neither_summarises_to_nothing() {
        assert_eq!(
            parse(r#"{"resourceType":"CapabilityStatement","status":"active"}"#).summary(),
            None,
            "the shell then says the root answered without saying what it is"
        );
    }

    #[test]
    fn the_rest_of_the_statement_is_ignored_rather_than_re_modelled() {
        let statement = parse(
            r#"{"fhirVersion":"6.0.0-ballot5","rest":[{"mode":"server","resource":[]}],
                "software":{"name":"FerroTERM"}}"#,
        );
        assert_eq!(
            statement.summary(),
            Some("FHIR 6.0.0-ballot5, FerroTERM".to_owned()),
            "the viewer carries the fields it renders and nothing more"
        );
    }
}

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
    /// `CapabilityStatement.rest`, which says what each resource type answers.
    #[serde(default)]
    rest: Vec<Rest>,
}

/// One `CapabilityStatement.rest`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct Rest {
    /// `rest.resource`, one per resource type this root serves.
    #[serde(default)]
    resource: Vec<RestResource>,
}

/// One `CapabilityStatement.rest.resource`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct RestResource {
    /// `resource.type`, the resource type it describes.
    r#type: Option<String>,
    /// `resource.operation`, the operations this root answers on that type.
    #[serde(default)]
    operation: Vec<RestOperation>,
}

/// One `CapabilityStatement.rest.resource.operation`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct RestOperation {
    /// `operation.name`, as the statement wrote it.
    name: Option<String>,
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

    /// Whether this root declares `operation` on `resource_type`.
    ///
    /// An affordance that runs an operation appears only where the statement
    /// declares it, so a screen never offers a run the root then refuses. A
    /// leading `$` is trimmed from both sides of the comparison: R4B's own
    /// example instances write the bare name
    /// (<https://hl7.org/fhir/R4B/capabilitystatement-example.json.html>) and
    /// a statement that writes `$translate` means the same operation.
    pub(crate) fn declares_operation(&self, resource_type: &str, operation: &str) -> bool {
        let wanted = operation.trim_start_matches('$');
        self.rest
            .iter()
            .flat_map(|rest| rest.resource.iter())
            .filter(|resource| resource.r#type.as_deref() == Some(resource_type))
            .flat_map(|resource| resource.operation.iter())
            .filter_map(|declared| declared.name.as_deref())
            .any(|name| name.trim_start_matches('$') == wanted)
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
    fn an_operation_is_declared_only_where_the_statement_names_it() {
        let statement = parse(
            r#"{"resourceType":"CapabilityStatement","rest":[{"mode":"server","resource":[
                {"type":"ConceptMap","operation":[{"name":"translate"}]},
                {"type":"ValueSet","operation":[{"name":"expand"}]}]}]}"#,
        );
        assert!(statement.declares_operation("ConceptMap", "translate"));
        assert!(
            statement.declares_operation("ConceptMap", "$translate"),
            "a statement that writes the bare name means the same operation"
        );
        assert!(
            !statement.declares_operation("ValueSet", "translate"),
            "an operation declared on one type is not declared on another"
        );
        assert!(!statement.declares_operation("CodeSystem", "lookup"));
    }

    #[test]
    fn a_statement_that_declares_no_rest_offers_no_operation() {
        assert!(
            !parse(r#"{"fhirVersion":"4.3.0"}"#).declares_operation("ConceptMap", "translate"),
            "the screen then says the root does not declare the operation"
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

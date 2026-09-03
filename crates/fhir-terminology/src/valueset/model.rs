//! The version-neutral `ValueSet`: identity and `compose`
//! (<https://hl7.org/fhir/R4B/valueset.html>).

use crate::compose::Compose;

/// What the compose layer and the operations need of a `ValueSet`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueSetModel {
    /// `url`.
    pub url: String,
    /// `version`.
    pub version: Option<String>,
    /// The supplements the value set asks for (`valueset-supplement`
    /// extension, canonicals).
    pub supplements: Vec<String>,
    /// The `structuredefinition-standards-status` extension's code, when set.
    pub standards_status: Option<String>,
    /// `name`.
    pub name: Option<String>,
    /// `title`.
    pub title: Option<String>,
    /// `status`.
    pub status: String,
    /// `experimental`.
    pub experimental: Option<bool>,
    /// `date`.
    pub date: Option<String>,
    /// `publisher`.
    pub publisher: Option<String>,
    /// `description`.
    pub description: Option<String>,
    /// `immutable`.
    pub immutable: Option<bool>,
    /// `compose`, empty when the resource has none.
    pub compose: Compose,
}

impl crate::versioned::Versioned for ValueSetModel {
    fn url(&self) -> &str {
        &self.url
    }

    fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
}

/// A `ValueSet` the model cannot represent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    /// The resource has no `url`.
    #[error("the ValueSet has no url")]
    NoUrl,
    /// A filter names an operator the specification does not define.
    #[error("filter on `{property}` uses unknown operator `{op}`")]
    FilterOperator {
        /// `filter.property`.
        property: String,
        /// `filter.op`.
        op: String,
    },
}

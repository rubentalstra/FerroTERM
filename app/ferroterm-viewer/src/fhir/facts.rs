//! One declared fact of a published resource, and how a value reads.
//!
//! A screen that draws a published definitional resource draws a list of
//! terms with values beside them. Which elements a resource type declares
//! differs; how a blank, absent, or boolean value reads does not, so those
//! rules are written once here and every reader shares them.

/// One fact of a published resource: a term and the value beside it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Fact {
    /// The term a reader reads.
    pub(crate) label: &'static str,
    /// The value, absent when the resource declared none.
    pub(crate) value: Option<String>,
}

/// One fact, with a value that names nothing read as absent.
pub(crate) fn fact(label: &'static str, value: Option<&str>) -> Fact {
    Fact {
        label,
        value: named(value).map(str::to_owned),
    }
}

/// A declared boolean as the word a reader reads.
///
/// The word carries the meaning, so nothing that renders one depends on a
/// colour or a glyph (<https://www.w3.org/TR/WCAG22/#use-of-color>).
pub(crate) fn flag(declared: Option<bool>) -> Option<&'static str> {
    declared.map(|value| if value { "yes" } else { "no" })
}

/// The trimmed text, or `None` when it names nothing.
pub(crate) fn named(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|trimmed| !trimmed.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_that_is_only_whitespace_names_nothing() {
        assert_eq!(named(Some("  ")), None);
        assert_eq!(named(Some(" active ")), Some("active"));
        assert_eq!(named(None), None);
    }

    #[test]
    fn a_declared_boolean_renders_as_a_word() {
        assert_eq!(flag(Some(true)), Some("yes"));
        assert_eq!(flag(Some(false)), Some("no"));
        assert_eq!(
            flag(None),
            None,
            "an undeclared boolean is not the same claim as a declared `no`"
        );
    }

    #[test]
    fn a_blank_fact_carries_its_label_and_no_value() {
        assert_eq!(
            fact("Status", Some("")),
            Fact {
                label: "Status",
                value: None,
            },
            "the row is still drawn, so the screen can state the absence"
        );
    }
}

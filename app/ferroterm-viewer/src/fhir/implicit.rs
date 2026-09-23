//! The implicit value set forms a code system defines for itself.
//!
//! A code system may publish value sets that are not resources but URLs, so
//! `[system]?fhir_vs=[form]` names a selection the server resolves without
//! anything being stored. SNOMED CT defines `isa/[sctid]`, `refset/[sctid]`,
//! and `ecl/[ecl]` (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Value
//! Sets"), and each of the three is the URL shorthand for one
//! `compose.include.filter` over a declared filter property.
//!
//! That is what keeps the composer neutral. A form is offered for a code
//! system only when the served version's `TerminologyCapabilities` declares
//! the filter property and operator the form is shorthand for
//! (<https://hl7.org/fhir/R4B/terminologycapabilities.html>), so a system that
//! declares the same filters gets the same forms and one that declares none
//! gets none. Nothing here names a code system.

use crate::fhir::terminology::FilterRow;
use crate::url::encode_query_component;

/// The query parameter a code system's implicit value sets are named with.
pub(crate) const FHIR_VS: &str = "fhir_vs";

/// One implicit value set form, and the declared filter it is shorthand for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Form {
    /// The keyword the form is spelled with after `fhir_vs=`.
    pub(crate) keyword: &'static str,
    /// The `CodeSystem.filter.code` the form selects on.
    pub(crate) property: &'static str,
    /// The `CodeSystem.filter.operator` the form applies.
    pub(crate) operator: &'static str,
    /// What the form is called on the screen.
    pub(crate) label: &'static str,
    /// What the value beside it is.
    pub(crate) value_label: &'static str,
    /// Whether the value is an expression whose position a refusal can point
    /// into, which is what makes the screen mark the offending character.
    pub(crate) expression: bool,
}

/// The forms the viewer knows how to spell.
///
/// The spelling is the specification's; whether a form applies to a system is
/// decided by that system's own declared filters and never by this table.
const FORMS: [Form; 3] = [
    Form {
        keyword: "isa",
        property: "concept",
        operator: "is-a",
        label: "Everything under one concept",
        value_label: "The concept code",
        expression: false,
    },
    Form {
        keyword: "refset",
        property: "concept",
        operator: "in",
        label: "The members of one reference set",
        value_label: "The reference set code",
        expression: false,
    },
    Form {
        keyword: "ecl",
        property: "constraint",
        operator: "=",
        label: "An expression constraint",
        value_label: "The expression",
        expression: true,
    },
];

/// The forms a version declaring `filters` offers.
///
/// A filter row is `TerminologyCapabilities.codeSystem.version.filter`, which
/// carries the property code and the operators the version supports on it, so
/// this is the capability statement deciding what the screen draws.
pub(crate) fn offered(filters: &[FilterRow]) -> Vec<Form> {
    FORMS
        .into_iter()
        .filter(|form| {
            filters.iter().any(|declared| {
                declared.code == form.property
                    && declared
                        .operators
                        .iter()
                        .any(|operator| operator == form.operator)
            })
        })
        .collect()
}

/// The form spelled `keyword`, when it is one the viewer knows.
pub(crate) fn form(keyword: &str) -> Option<Form> {
    FORMS.into_iter().find(|form| form.keyword == keyword)
}

impl Form {
    /// The canonical `system` and `value` make in this form.
    ///
    /// The value is percent-encoded, because the server reads the argument
    /// back through a URI decode and an expression constraint carries `|`,
    /// `<`, and `#`, every one of which is structural in a URL
    /// (<https://www.rfc-editor.org/rfc/rfc3986>). A system whose own
    /// canonical already carries a query string gets another parameter rather
    /// than a second `?`.
    pub(crate) fn canonical(self, system: &str, value: &str) -> String {
        let separator = if system.contains('?') { '&' } else { '?' };
        format!(
            "{system}{separator}{FHIR_VS}={}/{}",
            self.keyword,
            encode_query_component(value)
        )
    }
}

/// The system, the form, and the value a canonical in an implicit form names.
///
/// A canonical the reader typed, or one read back off a saved value set, is
/// read here so the builder reopens on the form that wrote it. A canonical in
/// no form the viewer knows answers `None`, which leaves it as typed text.
pub(crate) fn read(canonical: &str) -> Option<(String, Form, String)> {
    let (system, query) = canonical.split_once('?')?;
    let selector = query
        .split('&')
        .find_map(|pair| pair.strip_prefix(&format!("{FHIR_VS}=")))?;
    let (keyword, value) = selector.split_once('/')?;
    let form = form(keyword)?;
    Some((
        system.to_owned(),
        form,
        crate::url::decode_query_component(value),
    ))
}

/// The byte offset the server's own diagnostic points at, when it states one.
///
/// No FHIR element carries a character position inside an operation input:
/// `OperationOutcome.issue.expression` is a `FHIRPath` into the resource, not an
/// offset into a value (<https://hl7.org/fhir/R4B/operationoutcome.html>). So
/// this is our own design: the server states the position in its own words,
/// and the screen reads it back to mark the character. The outcome is rendered
/// verbatim beside the mark either way, and a diagnostic stating no position
/// marks nothing.
pub(crate) fn position_in(diagnostic: &str) -> Option<u32> {
    let mut words = diagnostic.split_whitespace();
    while let Some(word) = words.next() {
        if word == "byte" {
            let digits: String = words
                .next()?
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            return digits.parse().ok();
        }
    }
    None
}

/// `expression` split around the character at `position`, for marking it.
///
/// The offset is a byte offset into the expression, so the split lands on the
/// character boundary at or before it and the marked run is one whole
/// character. An offset past the end marks the end, which is where a
/// diagnostic about a truncated expression points.
pub(crate) fn mark(expression: &str, position: u32) -> (String, String, String) {
    let offset = usize::try_from(position).unwrap_or(usize::MAX);
    if offset >= expression.len() {
        return (expression.to_owned(), String::from(" "), String::new());
    }
    let start = expression
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= offset)
        .last()
        .unwrap_or_default();
    let (before, rest) = expression
        .split_at_checked(start)
        .unwrap_or((expression, ""));
    let mut characters = rest.chars();
    let at: String = characters.by_ref().take(1).collect();
    (before.to_owned(), at, characters.collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A declared filter, as `TerminologyCapabilities` carries one.
    fn declared(code: &str, operators: &[&str]) -> FilterRow {
        FilterRow {
            code: code.to_owned(),
            operators: operators
                .iter()
                .map(|operator| (*operator).to_owned())
                .collect(),
        }
    }

    #[test]
    fn a_version_declaring_no_filter_offers_no_form() {
        assert!(
            offered(&[]).is_empty(),
            "a form appears only where the capability statement declares the filter behind it"
        );
    }

    #[test]
    fn each_form_needs_its_own_property_and_operator() {
        let hierarchy = offered(&[declared("concept", &["is-a"])]);
        assert_eq!(
            hierarchy
                .iter()
                .map(|form| form.keyword)
                .collect::<Vec<_>>(),
            vec!["isa"],
            "the `in` operator is not declared, so the reference set form is not offered"
        );
        let both = offered(&[declared("concept", &["is-a", "in"])]);
        assert_eq!(
            both.iter().map(|form| form.keyword).collect::<Vec<_>>(),
            vec!["isa", "refset"]
        );
        let constrained = offered(&[declared("constraint", &["="])]);
        assert_eq!(
            constrained
                .iter()
                .map(|form| form.keyword)
                .collect::<Vec<_>>(),
            vec!["ecl"]
        );
    }

    #[test]
    fn an_expression_is_encoded_into_the_canonical_it_travels_in() {
        let ecl = form("ecl").expect("the expression form is one of the three");
        let canonical = ecl.canonical("https://terminology.example/x", "<<73211009 |diabetes|");
        assert_eq!(
            canonical, "https://terminology.example/x?fhir_vs=ecl/%3C%3C73211009%20%7Cdiabetes%7C",
            "every character that is structural in a URL is escaped"
        );
        assert_eq!(
            read(&canonical),
            Some((
                String::from("https://terminology.example/x"),
                ecl,
                String::from("<<73211009 |diabetes|"),
            )),
            "the builder reopens on the form and the value that wrote the canonical"
        );
    }

    #[test]
    fn a_system_that_already_carries_a_query_gains_a_parameter() {
        let isa = form("isa").expect("the hierarchy form is one of the three");
        assert_eq!(
            isa.canonical("https://terminology.example/x?edition=a", "12345"),
            "https://terminology.example/x?edition=a&fhir_vs=isa/12345",
            "a second `?` would not be a URL"
        );
    }

    #[test]
    fn the_position_comes_from_the_server_s_own_diagnostic() {
        assert_eq!(
            position_in(
                "implicit value set `x` is malformed: expected a focus concept at byte 17, found \"OR\""
            ),
            Some(17),
            "the parser's own offset is what the mark sits on"
        );
        assert_eq!(
            position_in("the expression nests 41 deep at byte 8; the limit is 40"),
            Some(8),
            "a trailing separator does not make the number unreadable"
        );
        assert_eq!(
            position_in("code `x` is not in code system `y`"),
            None,
            "a refusal that states no position marks nothing"
        );
        assert_eq!(position_in("failed at byte"), None);
        assert_eq!(position_in("failed at byte nowhere"), None);
    }

    #[test]
    fn the_marked_character_is_one_whole_character() {
        assert_eq!(
            mark("<<73211009", 2),
            (
                String::from("<<"),
                String::from("7"),
                String::from("3211009")
            )
        );
        let multibyte = "<<73211009 |diabetes mellitus\u{2014}type 2|";
        let (before, at, after) = mark(multibyte, 30);
        assert_eq!(
            format!("{before}{at}{after}"),
            multibyte,
            "the three parts are the expression, whole"
        );
        assert_eq!(at.chars().count(), 1, "one character is marked: `{at}`");
        let (before, at, after) = mark("<<7", 99);
        assert_eq!(before, "<<7", "an offset past the end marks the end");
        assert_eq!(at, " ");
        assert!(after.is_empty());
    }

    #[test]
    fn a_canonical_in_no_form_this_viewer_knows_is_left_as_it_is() {
        assert_eq!(read("https://terminology.example/vs/all"), None);
        assert_eq!(
            read("https://terminology.example/x?fhir_vs=everything"),
            None
        );
        assert_eq!(
            read("https://terminology.example/x?fhir_vs=made-up/1"),
            None
        );
        assert_eq!(form("made-up"), None);
    }
}

//! The XML reading shared by the two readers.

use quick_xml::XmlVersion;
use quick_xml::events::{BytesRef, BytesStart};

/// The normalized value of the attribute `name` on `event`.
pub(crate) fn attribute(event: &BytesStart<'_>, name: &str) -> Option<String> {
    event
        .attributes()
        .flatten()
        .find(|a| a.key.as_ref() == name)
        .and_then(|a| a.normalized_value(XmlVersion::Explicit1_0).ok())
        .map(std::borrow::Cow::into_owned)
}

/// The text a character or entity reference stands for.
///
/// Numeric references and the five predefined entities resolve; any other
/// entity is kept as written, so a document-defined entity stays visible
/// instead of vanishing.
pub(crate) fn reference(event: &BytesRef<'_>) -> String {
    if let Ok(Some(c)) = event.resolve_char_ref() {
        return c.to_string();
    }
    match &**event {
        "amp" => String::from("&"),
        "lt" => String::from("<"),
        "gt" => String::from(">"),
        "quot" => String::from("\""),
        "apos" => String::from("'"),
        other => format!("&{other};"),
    }
}

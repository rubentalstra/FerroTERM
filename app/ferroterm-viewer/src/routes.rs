//! The in-application addresses the shell links to.
//!
//! Every link keeps the selected FHIR version in the query, so a link a reader
//! copies reproduces exactly what they were looking at.

use crate::fhir::version::FhirVersion;
use crate::url::RequestUrl;

/// Where the server mounts the bundle, and where the router is based.
pub(crate) const UI_BASE: &str = "/ui";

/// The query parameter that carries the selected FHIR version.
pub(crate) const VERSION_PARAM: &str = "fhir";

/// Builds a link to one of the viewer's own pages.
///
/// `path` is the page's path below the base, with `""` naming the index.
pub(crate) fn ui_link(path: &str, version: FhirVersion) -> String {
    let mut url = RequestUrl::new().segment(UI_BASE.trim_start_matches('/'));
    for part in path.split('/').filter(|part| !part.is_empty()) {
        url = url.segment(part);
    }
    url.query(VERSION_PARAM, version.segment()).render("")
}

/// Rewrites the address a reader is on to select another FHIR version.
///
/// The switcher stays on the page the reader is reading, so switching version
/// is a navigation within the same route rather than a jump to the index.
pub(crate) fn version_link(pathname: &str, version: FhirVersion) -> String {
    format!(
        "{pathname}?{VERSION_PARAM}={version_segment}",
        version_segment = version.segment()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_link_carries_the_version() {
        assert_eq!(ui_link("", FhirVersion::R4B), "/ui?fhir=r4b");
    }

    #[test]
    fn a_page_link_carries_the_version() {
        assert_eq!(ui_link("settings", FhirVersion::R5), "/ui/settings?fhir=r5");
    }

    #[test]
    fn a_nested_page_link_keeps_its_segments() {
        assert_eq!(
            ui_link("/systems/detail", FhirVersion::R4),
            "/ui/systems/detail?fhir=r4",
            "leading and repeated separators do not produce empty segments"
        );
    }

    #[test]
    fn switching_version_keeps_the_page_the_reader_is_on() {
        assert_eq!(
            version_link("/ui/settings", FhirVersion::R6),
            "/ui/settings?fhir=r6"
        );
    }
}

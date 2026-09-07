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

/// The path a code system's own screen sits under.
pub(crate) const SYSTEMS_PATH: &str = "systems";

/// The concept browser's path below the base.
pub(crate) const BROWSE_PATH: &str = "browse";

/// The expansion runner's path below the base.
pub(crate) const EXPAND_PATH: &str = "expand";

/// The `$validate-code` and `$subsumes` runners' path below the base.
pub(crate) const VALIDATE_PATH: &str = "validate";

/// The value set screen's path below the base.
pub(crate) const VALUE_SETS_PATH: &str = "valuesets";

/// The concept map screen's path below the base.
pub(crate) const CONCEPT_MAPS_PATH: &str = "conceptmaps";

/// The version comparison screen's path below the base.
pub(crate) const VERSIONS_PATH: &str = "versions";

/// The settings screen's path below the base.
pub(crate) const SETTINGS_PATH: &str = "settings";

/// The overview's path below the base, which is the base itself.
pub(crate) const OVERVIEW_PATH: &str = "";

/// The query parameter that carries a code system canonical into a screen.
pub(crate) const SYSTEM_PARAM: &str = "system";

/// The query parameter that carries a code system version into a screen.
pub(crate) const SYSTEM_VERSION_PARAM: &str = "version";

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

/// Builds the link to one code system's own screen.
///
/// The canonical is one percent-encoded path segment, so a URI that carries
/// its own query string, or any other structural character, survives the
/// route: `leptos_router` splits the raw path on a literal `/` and only then
/// decodes the segment it captured.
// NOTE: a click navigation runs the path through `decodeURI`, which keeps the
// reserved set escaped (<https://tc39.es/ecma262/#sec-decodeuri-encodeduri>),
// so a canonical carrying a literal `%` is the one shape that does not survive.
pub(crate) fn system_link(system: &str, version: FhirVersion) -> String {
    RequestUrl::new()
        .segment(UI_BASE.trim_start_matches('/'))
        .segment(SYSTEMS_PATH)
        .segment(system)
        .query(VERSION_PARAM, version.segment())
        .render("")
}

/// Builds the link into a screen that works over one code system.
///
/// The system, and the code system version when one is declared, travel as
/// query parameters, so the address the reader lands on says exactly what it
/// is showing.
pub(crate) fn system_tool_link(
    path: &str,
    system: &str,
    system_version: Option<&str>,
    version: FhirVersion,
) -> String {
    let mut url = RequestUrl::new()
        .segment(UI_BASE.trim_start_matches('/'))
        .segment(path)
        .query(VERSION_PARAM, version.segment())
        .query(SYSTEM_PARAM, system);
    if let Some(code) = system_version {
        url = url.query(SYSTEM_VERSION_PARAM, code);
    }
    url.render("")
}

/// Builds the link that opens one value set canonical in the expansion runner.
///
/// The runner takes the canonical as its `url` parameter, so the link names
/// that and nothing else: every other parameter is the runner's own default
/// or the reader's stored page size.
pub(crate) fn expansion_link(canonical: &str, version: FhirVersion) -> String {
    RequestUrl::new()
        .segment(UI_BASE.trim_start_matches('/'))
        .segment(EXPAND_PATH)
        .query(VERSION_PARAM, version.segment())
        .query("url", canonical)
        .render("")
}

/// The sidebar entry a browser address belongs under, as its path segment.
///
/// `pathname` is the raw address the browser is on, base included, because
/// `use_location().pathname` never strips the router base. An address with no
/// sidebar entry answers `None`, so a screen a reader reached some other way
/// leaves every entry unmarked.
pub(crate) fn nav_section(pathname: &str) -> Option<&'static str> {
    let mut segments = pathname.split('/').filter(|segment| !segment.is_empty());
    if segments.next() != Some(UI_BASE.trim_start_matches('/')) {
        return None;
    }
    match segments.next() {
        // A code system's own screen is reached from the overview that lists
        // the systems, so the overview stays marked while a reader reads one.
        None | Some(SYSTEMS_PATH) => Some(OVERVIEW_PATH),
        Some(BROWSE_PATH) => Some(BROWSE_PATH),
        Some(EXPAND_PATH) => Some(EXPAND_PATH),
        Some(VALIDATE_PATH) => Some(VALIDATE_PATH),
        Some(VALUE_SETS_PATH) => Some(VALUE_SETS_PATH),
        Some(CONCEPT_MAPS_PATH) => Some(CONCEPT_MAPS_PATH),
        Some(VERSIONS_PATH) => Some(VERSIONS_PATH),
        Some(SETTINGS_PATH) => Some(SETTINGS_PATH),
        Some(_unlisted) => None,
    }
}

/// Rewrites the address a reader is on to select another FHIR version.
///
/// The switcher stays on the page the reader is reading, so switching version
/// is a navigation within the same route rather than a jump to the index.
/// Every other parameter the address carries is kept, because a screen that
/// works over one code system carries it in the query and switching version
/// must not drop what the reader is looking at. `search` is the query without
/// its leading `?`, already percent-encoded, so each pair is carried verbatim.
pub(crate) fn version_link(pathname: &str, search: &str, version: FhirVersion) -> String {
    let mut kept: Vec<&str> = search
        .split('&')
        .filter(|pair| {
            !pair.is_empty()
                && pair.split_once('=').map_or(*pair, |(name, _)| name) != VERSION_PARAM
        })
        .collect();
    let selected = format!("{VERSION_PARAM}={}", version.segment());
    kept.push(&selected);
    format!("{pathname}?{}", kept.join("&"))
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

    /// The segment `system_link` wrote, as `leptos_router` reads it back.
    ///
    /// The router splits the raw path on a literal `/` and decodes the
    /// captured segment afterwards, so this models both halves: take the
    /// segment, then percent-decode it.
    fn round_trip(link: &str) -> String {
        let below = link
            .strip_prefix("/ui/systems/")
            .expect("the link addresses a code system screen");
        let segment = below.split('?').next().unwrap_or_default();
        percent_encoding::percent_decode_str(segment)
            .decode_utf8()
            .expect("the encoder emits UTF-8")
            .into_owned()
    }

    #[test]
    fn a_code_system_canonical_round_trips_through_the_route() {
        let system = "https://terminology.example/animals";
        let link = system_link(system, FhirVersion::R4B);
        assert_eq!(
            link,
            "/ui/systems/https:%2F%2Fterminology.example%2Fanimals?fhir=r4b"
        );
        assert_eq!(round_trip(&link), system);
    }

    #[test]
    fn a_canonical_that_carries_a_query_string_round_trips_too() {
        let system = "https://terminology.example/x?fhir_vs=isa/12345&lang=nl";
        let link = system_link(system, FhirVersion::R6);
        assert_eq!(
            link.matches('?').count(),
            1,
            "the only `?` in the link is the one that opens the version query: {link}"
        );
        assert_eq!(round_trip(&link), system);
    }

    #[test]
    fn a_tool_link_carries_the_system_and_the_version() {
        assert_eq!(
            system_tool_link(
                EXPAND_PATH,
                "https://terminology.example/x",
                Some("2031-01-01"),
                FhirVersion::R5
            ),
            "/ui/expand?fhir=r5&system=https%3A%2F%2Fterminology.example%2Fx&version=2031-01-01"
        );
    }

    #[test]
    fn a_tool_link_for_a_system_with_no_declared_version_omits_it() {
        assert_eq!(
            system_tool_link(
                BROWSE_PATH,
                "https://terminology.example/x",
                None,
                FhirVersion::R4
            ),
            "/ui/browse?fhir=r4&system=https%3A%2F%2Fterminology.example%2Fx",
            "an absent version is left out rather than sent as an empty one"
        );
    }

    #[test]
    fn an_expansion_link_carries_the_canonical_the_runner_reads() {
        assert_eq!(
            expansion_link(
                "http://terminology.example/x?fhir_vs=isa/1",
                FhirVersion::R4B
            ),
            "/ui/expand?fhir=r4b&url=http%3A%2F%2Fterminology.example%2Fx%3Ffhir_vs%3Disa%2F1",
            "an implicit canonical carrying its own query string stays in one parameter"
        );
    }

    #[test]
    fn the_base_itself_is_the_overview() {
        assert_eq!(nav_section("/ui"), Some(OVERVIEW_PATH));
        assert_eq!(
            nav_section("/ui/"),
            Some(OVERVIEW_PATH),
            "the index is reachable with and without its trailing separator"
        );
    }

    #[test]
    fn a_screen_marks_its_own_entry() {
        assert_eq!(nav_section("/ui/browse"), Some(BROWSE_PATH));
        assert_eq!(nav_section("/ui/expand"), Some(EXPAND_PATH));
        assert_eq!(nav_section("/ui/validate"), Some(VALIDATE_PATH));
        assert_eq!(nav_section("/ui/valuesets"), Some(VALUE_SETS_PATH));
        assert_eq!(nav_section("/ui/conceptmaps"), Some(CONCEPT_MAPS_PATH));
        assert_eq!(nav_section("/ui/versions"), Some(VERSIONS_PATH));
        assert_eq!(nav_section("/ui/settings"), Some(SETTINGS_PATH));
    }

    #[test]
    fn a_code_system_screen_marks_the_overview_that_lists_it() {
        assert_eq!(
            nav_section("/ui/systems/https:%2F%2Fterminology.example%2Fanimals"),
            Some(OVERVIEW_PATH),
            "a reader reading one system is still under the screen that listed it"
        );
    }

    #[test]
    fn an_address_with_no_entry_marks_nothing() {
        assert_eq!(nav_section("/ui/nowhere"), None);
        assert_eq!(
            nav_section("/health"),
            None,
            "an address outside the base is not one of the viewer's screens"
        );
    }

    #[test]
    fn switching_version_keeps_the_page_the_reader_is_on() {
        assert_eq!(
            version_link("/ui/settings", "", FhirVersion::R6),
            "/ui/settings?fhir=r6"
        );
    }

    #[test]
    fn switching_version_keeps_every_other_parameter_the_address_carries() {
        assert_eq!(
            version_link(
                "/ui/expand",
                "fhir=r4b&system=https%3A%2F%2Fterminology.example%2Fx&version=2031-01-01",
                FhirVersion::R5
            ),
            "/ui/expand?system=https%3A%2F%2Fterminology.example%2Fx&version=2031-01-01&fhir=r5",
            "a switcher that dropped the system would change what the screen shows"
        );
    }

    #[test]
    fn switching_version_does_not_leave_the_version_it_replaced_behind() {
        let rewritten = version_link("/ui/expand", "fhir=r4&fhir=r5&q=a", FhirVersion::R6);
        assert_eq!(rewritten.matches("fhir=").count(), 1, "{rewritten}");
        assert_eq!(rewritten, "/ui/expand?q=a&fhir=r6");
    }

    #[test]
    fn a_parameter_without_a_value_survives_the_rewrite() {
        assert_eq!(
            version_link("/ui/browse", "flat&fhir=r4", FhirVersion::R4B),
            "/ui/browse?flat&fhir=r4b",
            "the query is carried verbatim, so a bare parameter is not invented into a pair"
        );
    }
}

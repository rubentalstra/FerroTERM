//! The part of a `TerminologyCapabilities` the overview renders.
//!
//! `GET [base]/metadata?mode=terminology` is the one document that says what a
//! deployment loaded (<https://hl7.org/fhir/R4B/terminologycapabilities.html>).
//! Reading it lives here, outside every component, so the screen is a
//! rendering of a value that plain unit tests can pin.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The canonical FerroTERM declares a served version's artifact under.
///
/// No FHIR specification records which index a server read, so the server
/// states it as an extension on `codeSystem.version`, which is where it
/// belongs: one artifact holds one code system version, and two editions of
/// one system share a single `codeSystem` entry.
const ARTIFACT_EXTENSION: &str =
    "https://ferroterm.eu/fhir/StructureDefinition/terminology-artifact";

/// The sub-extension carrying the artifact directory's own name.
const ARTIFACT_NAME: &str = "name";

/// The sub-extension carrying the release identifier the build recorded.
const ARTIFACT_RELEASE: &str = "release";

/// What `GET [base]/metadata?mode=terminology` declares about a served root.
///
/// Every field is optional so a server that omits one still renders. The
/// viewer carries the elements it draws and nothing else: it never mirrors the
/// whole resource.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct TerminologyCapabilities {
    /// `TerminologyCapabilities.codeSystem`, one entry per served system.
    #[serde(default, rename = "codeSystem")]
    code_system: Vec<CodeSystem>,
}

/// One `TerminologyCapabilities.codeSystem`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct CodeSystem {
    /// `codeSystem.uri`, the canonical of the system.
    uri: Option<String>,
    /// `codeSystem.content`, which R5 and R6 declare and R4 and R4B do not.
    content: Option<String>,
    /// `codeSystem.subsumption`, whether the server answers `$subsumes`.
    subsumption: Option<bool>,
    /// `codeSystem.version`, one entry per served version.
    #[serde(default)]
    version: Vec<Version>,
}

/// One `TerminologyCapabilities.codeSystem.version`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct Version {
    /// `version.code` in R4, R4B, and R5.
    code: Option<String>,
    /// `version.value`, which is what the R6 ballot renamed `code` to.
    value: Option<String>,
    /// `version.isDefault`, the version an unversioned request resolves to.
    #[serde(rename = "isDefault")]
    is_default: Option<bool>,
    /// `version.compositional`, whether the server reads the system's grammar.
    compositional: Option<bool>,
    /// `version.language`, the designation languages the server holds.
    #[serde(default)]
    language: Vec<String>,
    /// `version.filter`, the filters `$expand` accepts for this version.
    #[serde(default)]
    filter: Vec<Filter>,
    /// `version.property`, the properties `$lookup` answers for this version.
    #[serde(default)]
    property: Vec<String>,
    /// The extensions the server declared on this version.
    #[serde(default)]
    extension: Vec<Extension>,
}

/// One `TerminologyCapabilities.codeSystem.version.filter`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct Filter {
    /// The filter property.
    code: Option<String>,
    /// The operators the server answers for it.
    #[serde(default)]
    op: Vec<String>,
}

/// One `Extension`, read only for the artifact declaration.
///
/// A complex extension carries its parts as child extensions
/// (<https://hl7.org/fhir/R4B/extensibility.html>), and the artifact carries
/// its two parts as `valueString`, which is the one value type read here.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct Extension {
    /// The canonical that says what the extension means.
    url: Option<String>,
    /// The parts of a complex extension.
    #[serde(default, rename = "extension")]
    parts: Vec<Extension>,
    /// `valueString`.
    #[serde(rename = "valueString")]
    value_string: Option<String>,
}

/// One code system, as the overview draws it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SystemCard {
    /// The system canonical, empty when the server declared none.
    pub(crate) url: String,
    /// The declared content mode, absent where the FHIR version has no such
    /// element.
    pub(crate) content: Option<String>,
    /// Whether the server answers subsumption for this system.
    pub(crate) subsumption: Option<bool>,
    /// The served versions, in the order the server declared them.
    pub(crate) versions: Vec<VersionRow>,
}

/// One served version of a code system, as the overview draws it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct VersionRow {
    /// The version identifier, absent when the server declared none.
    pub(crate) code: Option<String>,
    /// Whether an unversioned request resolves to this version.
    pub(crate) is_default: bool,
    /// Whether the server reads the system's compositional grammar.
    pub(crate) compositional: Option<bool>,
    /// The designation languages this version holds.
    pub(crate) languages: Vec<String>,
    /// The filters `$expand` accepts for this version.
    pub(crate) filters: Vec<FilterRow>,
    /// The properties `$lookup` answers for this version.
    pub(crate) properties: Vec<String>,
    /// The artifact this version was read from, when the server read one.
    pub(crate) artifact: Option<Artifact>,
}

/// The built index one served code system version was read from.
///
/// A system a deployment did not build an index for, such as a registry the
/// server carries or a `CodeSystem` resource posted through the API, declares
/// none, which is an ordinary answer rather than a missing one.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Artifact {
    /// The artifact directory's own name, without the directories above it.
    pub(crate) name: Option<String>,
    /// The release identifier the offline build recorded for it.
    pub(crate) release: Option<String>,
}

/// One declared filter and the operators it takes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct FilterRow {
    /// The filter property.
    pub(crate) code: String,
    /// The operator codes, as the server wrote them.
    pub(crate) operators: Vec<String>,
}

impl Version {
    /// The version identifier, from whichever element this FHIR version uses.
    ///
    /// R4, R4B, and R5 call it `code`; the R6 ballot renamed it `value`
    /// (<https://hl7.org/fhir/6.0.0-ballot5/terminologycapabilities.html>). A
    /// version declared as the empty string named nothing, so it reads as
    /// absent.
    fn identifier(&self) -> Option<String> {
        named(self.code.as_deref()).or_else(|| named(self.value.as_deref()))
    }
}

impl Extension {
    /// The value of the sub-extension `url` names, when it declared one.
    fn part(&self, url: &str) -> Option<String> {
        self.parts
            .iter()
            .find(|part| part.url.as_deref() == Some(url))
            .and_then(Extension::value)
    }

    /// The declared `valueString`, when the part carried one.
    fn value(&self) -> Option<String> {
        self.value_string.clone()
    }
}

impl TerminologyCapabilities {
    /// The cards the overview renders, one per distinct code system canonical.
    ///
    /// Cards are ordered by canonical, so the rendered list is the same on
    /// every read and every card carries a key no other card shares. A
    /// document that repeats a canonical merges the repeats into one card and
    /// keeps the first declaration of the system-level facts.
    pub(crate) fn cards(&self) -> Vec<SystemCard> {
        let mut cards: BTreeMap<&str, SystemCard> = BTreeMap::new();
        for system in &self.code_system {
            let url = system.uri.as_deref().unwrap_or_default();
            let card = cards.entry(url).or_insert_with(|| opened(url, system));
            add_versions(card, system);
        }
        cards.into_values().collect()
    }

    /// The card for one canonical, or `None` when this root declares no such
    /// system.
    ///
    /// A canonical the document does not name is an ordinary answer: a reader
    /// followed a link, switched FHIR version, or typed an address, and the
    /// screen says so rather than failing.
    pub(crate) fn card(&self, url: &str) -> Option<SystemCard> {
        let mut found: Option<SystemCard> = None;
        for system in self
            .code_system
            .iter()
            .filter(|system| system.uri.as_deref().unwrap_or_default() == url)
        {
            add_versions(found.get_or_insert_with(|| opened(url, system)), system);
        }
        found
    }
}

/// A card carrying `system`'s system-level facts and no version yet.
fn opened(url: &str, system: &CodeSystem) -> SystemCard {
    SystemCard {
        url: url.to_owned(),
        content: system.content.clone(),
        subsumption: system.subsumption,
        versions: Vec::new(),
    }
}

/// Appends every version `system` declares to `card`, in declaration order.
fn add_versions(card: &mut SystemCard, system: &CodeSystem) {
    card.versions.extend(system.version.iter().map(|version| {
        VersionRow {
            code: version.identifier(),
            is_default: version.is_default.unwrap_or_default(),
            compositional: version.compositional,
            languages: version.language.clone(),
            filters: version
                .filter
                .iter()
                .map(|filter| FilterRow {
                    code: filter.code.clone().unwrap_or_default(),
                    operators: filter.op.clone(),
                })
                .collect(),
            properties: version.property.clone(),
            artifact: artifact_of(&version.extension),
        }
    }));
}

/// The trimmed text, or `None` when it names nothing.
fn named(text: Option<&str>) -> Option<String> {
    text.map(str::trim)
        .filter(|trimmed| !trimmed.is_empty())
        .map(str::to_owned)
}

/// The artifact `extensions` declares, when the server declared one.
fn artifact_of(extensions: &[Extension]) -> Option<Artifact> {
    let declaration = extensions
        .iter()
        .find(|extension| extension.url.as_deref() == Some(ARTIFACT_EXTENSION))?;
    Some(Artifact {
        name: declaration.part(ARTIFACT_NAME),
        release: declaration.part(ARTIFACT_RELEASE),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The document `GET /r4/metadata?mode=terminology` answered.
    const R4: &str = include_str!("../../fixtures/terminology-capabilities-r4.json");
    /// The document `GET /r4b/metadata?mode=terminology` answered.
    const R4B: &str = include_str!("../../fixtures/terminology-capabilities-r4b.json");
    /// The document `GET /r5/metadata?mode=terminology` answered.
    const R5: &str = include_str!("../../fixtures/terminology-capabilities-r5.json");
    /// The document `GET /r6/metadata?mode=terminology` answered.
    const R6: &str = include_str!("../../fixtures/terminology-capabilities-r6.json");

    /// Every recorded document, named by the root that answered it.
    const RECORDED: [(&str, &str); 4] = [("r4", R4), ("r4b", R4B), ("r5", R5), ("r6", R6)];

    fn parse(json: &str) -> TerminologyCapabilities {
        serde_json::from_str(json).expect("the fixture is valid JSON")
    }

    fn card_of<'a>(cards: &'a [SystemCard], url: &str) -> &'a SystemCard {
        cards
            .iter()
            .find(|card| card.url == url)
            .expect("the fixture declares this system")
    }

    #[test]
    fn every_served_version_declares_the_same_systems() {
        let mut seen: Vec<(&str, Vec<String>)> = Vec::new();
        for (root, document) in RECORDED {
            let urls: Vec<String> = parse(document)
                .cards()
                .into_iter()
                .map(|card| card.url)
                .collect();
            assert!(
                urls.len() > 1,
                "{root} declares more than one system, so the screen is exercised"
            );
            seen.push((root, urls));
        }
        let first = seen.first().expect("four roots were recorded").clone();
        for (root, urls) in &seen {
            assert_eq!(
                urls, &first.1,
                "{root} declares the same systems as {}",
                first.0
            );
        }
    }

    #[test]
    fn the_cards_are_ordered_by_canonical_on_every_version() {
        for (root, document) in RECORDED {
            let urls: Vec<String> = parse(document)
                .cards()
                .into_iter()
                .map(|card| card.url)
                .collect();
            let mut sorted = urls.clone();
            sorted.sort();
            assert_eq!(urls, sorted, "{root} renders in a stable order");
        }
    }

    #[test]
    fn the_r6_version_identifier_is_read_from_its_renamed_element() {
        for (root, document) in RECORDED {
            let cards = parse(document).cards();
            let versions: Vec<Option<String>> = cards
                .iter()
                .flat_map(|card| card.versions.iter().map(|version| version.code.clone()))
                .collect();
            assert!(
                versions.iter().filter(|code| code.is_some()).count() > 1,
                "{root} names its versions, whichever element it uses"
            );
        }
    }

    #[test]
    fn the_default_version_is_marked() {
        for (root, document) in RECORDED {
            let cards = parse(document).cards();
            for card in &cards {
                assert_eq!(
                    card.versions
                        .iter()
                        .filter(|version| version.is_default)
                        .count(),
                    1,
                    "{root} marks exactly one default for {}",
                    card.url
                );
            }
        }
    }

    #[test]
    fn the_content_mode_is_declared_only_where_the_version_has_the_element() {
        let r4b = parse(R4B).cards();
        assert!(
            r4b.iter().all(|card| card.content.is_none()),
            "R4B has no codeSystem.content, so the screen states its absence"
        );
        let r5 = parse(R5).cards();
        assert!(
            r5.iter().all(|card| card.content.is_some()),
            "R5 declares a content mode for every system"
        );
    }

    #[test]
    fn subsumption_is_read_per_system_rather_than_assumed() {
        let cards = parse(R5).cards();
        let declared: Vec<Option<bool>> = cards.iter().map(|card| card.subsumption).collect();
        assert!(
            declared.contains(&Some(true)) && declared.contains(&Some(false)),
            "the recorded document has systems on both sides: {declared:?}"
        );
    }

    #[test]
    fn a_version_that_names_no_identifier_reads_as_absent() {
        let cards = parse(R6).cards();
        let card = card_of(&cards, "urn:ietf:bcp:13");
        assert_eq!(
            card.versions
                .first()
                .and_then(|version| version.code.clone()),
            None,
            "an empty version string names nothing, so the screen states the absence"
        );
    }

    #[test]
    fn an_empty_r6_code_element_does_not_hide_the_value_element() {
        let cards = parse(
            r#"{"codeSystem":[{"uri":"https://terminology.example/x",
                "version":[{"code":"","value":"2.0"}]}]}"#,
        )
        .cards();
        assert_eq!(
            card_of(&cards, "https://terminology.example/x")
                .versions
                .first()
                .and_then(|version| version.code.clone()),
            Some("2.0".to_owned()),
            "an element that names nothing does not shadow the one that does"
        );
    }

    #[test]
    fn a_version_that_declares_no_language_reads_as_an_empty_list() {
        let cards = parse(R4B).cards();
        let card = card_of(&cards, "urn:ietf:bcp:47");
        assert_eq!(
            card.versions.first().map(|version| version.languages.len()),
            Some(0),
            "the screen states the absence rather than drawing an empty row"
        );
    }

    #[test]
    fn a_declared_filter_keeps_its_operators() {
        let cards = parse(R5).cards();
        let card = card_of(&cards, "http://example.org/fhir/CodeSystem/animals");
        let filters = &card
            .versions
            .first()
            .expect("the fixture serves one version")
            .filters;
        let legs = filters
            .iter()
            .find(|filter| filter.code == "legs")
            .expect("the fixture declares a system-specific filter");
        assert_eq!(legs.operators, ["=", "in"]);
    }

    #[test]
    fn an_invented_code_system_renders_with_no_change_here() {
        let document = parse(
            r#"{"resourceType":"TerminologyCapabilities","codeSystem":[
                {"uri":"https://terminology.example/invented","content":"fragment",
                 "subsumption":false,
                 "version":[{"code":"2031-02-01","isDefault":true,"compositional":true,
                             "language":["cy"],
                             "filter":[{"code":"kind","op":["=","in"]}]}]}]}"#,
        );
        let cards = document.cards();
        let card = card_of(&cards, "https://terminology.example/invented");
        assert_eq!(card.content.as_deref(), Some("fragment"));
        assert_eq!(card.subsumption, Some(false));
        let version = card.versions.first().expect("one version was declared");
        assert_eq!(version.code.as_deref(), Some("2031-02-01"));
        assert!(version.is_default, "the only version is the default");
        assert_eq!(version.compositional, Some(true));
        assert_eq!(version.languages, ["cy"]);
        assert_eq!(
            version.filters.first().map(|filter| filter.code.clone()),
            Some("kind".to_owned()),
            "a system the server has never served before needs no code here"
        );
    }

    #[test]
    fn a_system_that_declares_nothing_still_renders_a_card() {
        let cards = parse(r#"{"codeSystem":[{"uri":"https://terminology.example/bare"}]}"#).cards();
        let card = card_of(&cards, "https://terminology.example/bare");
        assert_eq!(card.content, None);
        assert_eq!(card.subsumption, None);
        assert!(
            card.versions.is_empty(),
            "the screen states that no version was declared"
        );
    }

    #[test]
    fn a_system_without_a_canonical_is_kept_rather_than_dropped() {
        let cards = parse(r#"{"codeSystem":[{"version":[{"code":"1"}]}]}"#).cards();
        assert_eq!(cards.len(), 1, "a card the reader can see beats a silence");
        assert_eq!(
            cards.first().map(|card| card.url.clone()),
            Some(String::new()),
            "the screen states that the server named no canonical"
        );
    }

    #[test]
    fn a_repeated_canonical_merges_into_one_card() {
        let cards = parse(
            r#"{"codeSystem":[
                {"uri":"https://terminology.example/twice","version":[{"code":"1"}]},
                {"uri":"https://terminology.example/twice","version":[{"code":"2"}]}]}"#,
        )
        .cards();
        assert_eq!(
            cards.len(),
            1,
            "one canonical is one card, so keys are unique"
        );
        assert_eq!(
            cards
                .first()
                .map(|card| card.versions.len())
                .unwrap_or_default(),
            2,
            "no declared version is dropped by the merge"
        );
    }

    #[test]
    fn every_version_read_from_an_artifact_names_it_on_every_fhir_version() {
        for (root, document) in RECORDED {
            let cards = parse(document).cards();
            let card = card_of(&cards, "http://loinc.org");
            let version = card.versions.first().expect("one version is served");
            let artifact = version
                .artifact
                .as_ref()
                .expect("the recorded server read this system from an artifact");
            assert_eq!(
                (artifact.name.as_deref(), artifact.release.as_deref()),
                (Some("loinc"), Some("2.99")),
                "{root} names the artifact and the release the build recorded"
            );
        }
    }

    #[test]
    fn a_system_that_came_from_no_artifact_declares_none() {
        for (root, document) in RECORDED {
            let cards = parse(document).cards();
            let card = card_of(&cards, "urn:iso:std:iso:3166");
            assert!(
                card.versions
                    .iter()
                    .all(|version| version.artifact.is_none()),
                "{root}: a registry the server carries is loaded from no artifact"
            );
        }
    }

    #[test]
    fn an_artifact_that_names_no_directory_still_names_its_release() {
        let cards = parse(&format!(
            r#"{{"codeSystem":[{{"uri":"https://terminology.example/x","version":[{{"code":"1",
                "extension":[{{"url":"{ARTIFACT_EXTENSION}",
                  "extension":[{{"url":"{ARTIFACT_RELEASE}","valueString":"20260630"}}]}}]}}]}}]}}"#
        ))
        .cards();
        let artifact = card_of(&cards, "https://terminology.example/x")
            .versions
            .first()
            .and_then(|version| version.artifact.clone())
            .expect("the version declares an artifact");
        assert_eq!(artifact.name, None, "the screen states the absence");
        assert_eq!(artifact.release.as_deref(), Some("20260630"));
    }

    #[test]
    fn the_lookup_properties_are_read_per_version_on_every_fhir_version() {
        for (root, document) in RECORDED {
            let cards = parse(document).cards();
            let card = card_of(&cards, "http://example.org/fhir/CodeSystem/animals");
            let version = card
                .versions
                .first()
                .expect("the fixture serves one version");
            assert!(
                version.properties.contains(&"legs".to_owned()),
                "{root} declares the system's own `$lookup` property"
            );
        }
    }

    #[test]
    fn a_version_that_declares_no_property_reads_as_an_empty_list() {
        let cards = parse(
            r#"{"codeSystem":[{"uri":"https://terminology.example/bare",
            "version":[{"code":"1"}]}]}"#,
        )
        .cards();
        assert_eq!(
            card_of(&cards, "https://terminology.example/bare")
                .versions
                .first()
                .map(|version| version.properties.len()),
            Some(0),
            "the screen states the absence rather than drawing an empty list"
        );
    }

    #[test]
    fn a_canonical_the_document_names_is_found_as_one_card() {
        let document = parse(R5);
        let found = document
            .card("http://example.org/fhir/CodeSystem/animals")
            .expect("the fixture declares this system");
        assert_eq!(found.url, "http://example.org/fhir/CodeSystem/animals");
        assert!(!found.versions.is_empty(), "the card carries its versions");
    }

    #[test]
    fn a_repeated_canonical_is_one_card_whether_it_is_looked_up_or_listed() {
        let document = parse(
            r#"{"codeSystem":[
                {"uri":"https://terminology.example/twice","content":"complete",
                 "version":[{"code":"1"}]},
                {"uri":"https://terminology.example/twice","version":[{"code":"2"}]}]}"#,
        );
        assert_eq!(
            document.card("https://terminology.example/twice"),
            document
                .cards()
                .into_iter()
                .find(|card| card.url == "https://terminology.example/twice"),
            "one canonical reads the same whichever way the screen asks for it"
        );
    }

    #[test]
    fn a_canonical_the_document_does_not_name_is_absent_rather_than_an_error() {
        assert_eq!(
            parse(R5).card("https://terminology.example/never-served"),
            None,
            "the screen states the absence, and does not render an error page"
        );
    }

    #[test]
    fn a_document_with_no_code_system_reads_as_no_cards() {
        assert!(
            parse(r#"{"resourceType":"TerminologyCapabilities","status":"active"}"#)
                .cards()
                .is_empty(),
            "the screen then says the server declared no code system"
        );
    }
}

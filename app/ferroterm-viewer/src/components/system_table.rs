//! The served code systems as one table, one row per served version.
//!
//! A deployment serves twenty systems as readily as five, so the overview is
//! an inventory a reader scans rather than a card each. It carries the five
//! facts a reader picks a system by; the grammar, the designation languages,
//! the filters and the published resource are on the system's own screen, one
//! click from the row.
//!
//! Nothing here names a system, assumes a hierarchy, or assumes a language.
//! Every cell is the capability statement rendered, so a system this server
//! has never served draws correctly with no change to this file.

use leptos::prelude::*;

use crate::components::shell::SelectedVersion;
use crate::fhir::concept::Hierarchy;
use crate::fhir::terminology::Artifact;
use crate::fhir::terminology::SystemCard;
use crate::fhir::terminology::VersionRow;
use crate::routes::BROWSE_PATH;
use crate::routes::VALIDATE_PATH;
use crate::routes::system_link;
use crate::routes::system_tool_link;
use crate::styles;

/// The served systems, one row per version.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn SystemTable(
    /// The systems to draw, as the capability statement declared them.
    cards: Vec<SystemCard>,
) -> impl IntoView {
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let rows: Vec<AnyView> = cards
        .into_iter()
        .flat_map(|card| system_rows(&card, version))
        .collect();
    view! {
        <div class=format!("mt-3 overflow-x-auto {}", styles::PANEL)>
            <table class=styles::TABLE>
                <thead>
                    <tr>
                        <th scope="col" class=styles::TH>
                            "Code system"
                        </th>
                        <th scope="col" class=styles::TH>
                            "Version"
                        </th>
                        <th scope="col" class=styles::TH>
                            "Content"
                        </th>
                        <th scope="col" class=styles::TH>
                            "Subsumes"
                        </th>
                        <th scope="col" class=styles::TH>
                            "Read from"
                        </th>
                        <th scope="col" class=styles::TH>
                            "Open in"
                        </th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
        </div>
    }
}

/// Every row one system contributes: one per served version, or one saying the
/// server declared none.
fn system_rows(
    card: &SystemCard,
    version: Signal<crate::fhir::version::FhirVersion>,
) -> Vec<AnyView> {
    let content = card.content.clone();
    let subsumption = card.subsumption;
    if card.versions.is_empty() {
        return vec![row(
            card,
            version,
            None,
            content.as_deref(),
            subsumption,
            true,
        )];
    }
    card.versions
        .iter()
        .enumerate()
        .map(|(index, served)| {
            row(
                card,
                version,
                Some(served),
                content.as_deref(),
                subsumption,
                index == 0,
            )
        })
        .collect()
}

/// One row: the system on its first, an indent mark on the rest.
fn row(
    card: &SystemCard,
    version: Signal<crate::fhir::version::FhirVersion>,
    served: Option<&VersionRow>,
    content: Option<&str>,
    subsumption: Option<bool>,
    first: bool,
) -> AnyView {
    let system = system_cell(card, version, first);
    let code = served.and_then(|served| served.code.clone());
    let default = served.is_some_and(|served| served.is_default);
    let artifact = served.and_then(|served| served.artifact.clone());
    view! {
        <tr>
            <td class=styles::TD>{system}</td>
            <td class=styles::TD_TIGHT>{version_cell(code, default)}</td>
            <td class=styles::TD_TIGHT>{word_cell(content.map(str::to_owned))}</td>
            <td class=styles::TD_TIGHT>{yes_no_cell(subsumption)}</td>
            <td class=styles::TD>{artifact_cell(artifact.as_ref())}</td>
            <td class=styles::TD_TIGHT>{tools_cell(card, version, served)}</td>
        </tr>
    }
    .into_any()
}

/// The screens this version can be opened in.
///
/// A row hands its subject on rather than making a reader retype a canonical.
/// The browser is offered only where the version declares the direct-child
/// operator, because that is the one operator a tree walks
/// (<https://hl7.org/fhir/R5/codesystem-filter-operator.html>).
fn tools_cell(
    card: &SystemCard,
    version: Signal<crate::fhir::version::FhirVersion>,
    served: Option<&VersionRow>,
) -> AnyView {
    if card.url.is_empty() {
        return absent("a screen needs the canonical this system was declared without");
    }
    let code = served.and_then(|served| served.code.clone());
    let walkable = served.is_some_and(|served| Hierarchy::of(served).walk().is_some());
    let link = |path: &'static str, label: &'static str| {
        let system = card.url.clone();
        let code = code.clone();
        let href = move || system_tool_link(path, &system, code.as_deref(), version.get());
        view! {
            <a href=href class=styles::BUTTON_QUIET>
                {label}
            </a>
        }
        .into_any()
    };
    let browse = walkable.then(|| link(BROWSE_PATH, "Browse"));
    view! { <span class="flex gap-1">{browse} {link(VALIDATE_PATH, "Validate")}</span> }.into_any()
}

/// The system's canonical, as the link onto its screen, once per system.
fn system_cell(
    card: &SystemCard,
    version: Signal<crate::fhir::version::FhirVersion>,
    first: bool,
) -> AnyView {
    if !first {
        return absent("the version above belongs to the same code system");
    }
    if card.url.is_empty() {
        return view! {
            <span class=styles::CODE_MUTED>
                "This server declared a code system without a canonical URI."
            </span>
        }
        .into_any();
    }
    let target = card.url.clone();
    let href = move || system_link(&target, version.get());
    let label = card.url.clone();
    view! {
        <a href=href class=format!("{} {}", styles::CODE, styles::LINK)>
            {label}
        </a>
    }
    .into_any()
}

/// The served version, with the default marked.
fn version_cell(code: Option<String>, default: bool) -> AnyView {
    let Some(code) = code.filter(|code| !code.is_empty()) else {
        return absent("this server declared no version for the code system");
    };
    let mark = default.then(|| view! { <span class=styles::BADGE>"default"</span> }.into_any());
    view! {
        <span class=styles::CODE>{code}</span>
        {mark}
    }
    .into_any()
}

/// A word the server stated, or the mark for one it did not.
fn word_cell(word: Option<String>) -> AnyView {
    match word.filter(|word| !word.is_empty()) {
        Some(word) => view! { <span>{word}</span> }.into_any(),
        None => absent("this FHIR version has no such element, or the server declared none"),
    }
}

/// A flag the server stated, or the mark for one it did not.
fn yes_no_cell(flag: Option<bool>) -> AnyView {
    match flag {
        Some(true) => view! { <span>"yes"</span> }.into_any(),
        Some(false) => view! { <span class=styles::ABSENT>"no"</span> }.into_any(),
        None => absent("the server declared neither yes nor no"),
    }
}

/// The artifact a version was read from.
///
/// A system the deployment built no index for, such as a registry the server
/// carries or a `CodeSystem` posted through the API, was read from none. Only
/// the artifact's own name is on the wire, so no path is shown.
fn artifact_cell(artifact: Option<&Artifact>) -> AnyView {
    let Some(artifact) = artifact else {
        return absent("this version was not read from a built artifact");
    };
    let name = artifact.name.clone();
    let release = artifact.release.clone();
    let release_line =
        release.map(|release| view! { <span class=styles::HINT>{release}</span> }.into_any());
    match name {
        Some(name) => view! {
            <span class=format!("block {}", styles::CODE)>{name}</span>
            {release_line}
        }
        .into_any(),
        None => absent("the server named no artifact for this version"),
    }
}

/// The mark for a fact the server did not state, with the sentence behind it.
///
/// A cell that spells absence out in prose makes "not" the densest word on the
/// screen; the reader who wants the sentence hovers or focuses the mark.
fn absent(why: &'static str) -> AnyView {
    view! {
        <span class=styles::ABSENT title=why>
            {styles::ABSENT_MARK}
        </span>
    }
    .into_any()
}

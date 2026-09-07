//! One code system, drawn from what the capability statement declares.

use leptos::prelude::*;

use crate::components::NOT_DECLARED;
use crate::components::shell::SelectedVersion;
use crate::fhir::terminology::Artifact;
use crate::fhir::terminology::SystemCard;
use crate::fhir::terminology::VersionRow;
use crate::routes::system_link;

/// Draws one code system: its canonical, its versions, and what it supports.
///
/// Nothing here names a system, assumes a hierarchy, or assumes a language.
/// The card is the capability statement rendered, so a system this server has
/// never served before draws correctly with no change to this file.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn CodeSystemCard(
    /// The system to draw, as the capability statement declared it.
    card: SystemCard,
) -> impl IntoView {
    let heading = if card.url.is_empty() {
        view! {
            <h3 class="font-mono text-sm break-all text-slate-500 dark:text-slate-400">
                "This server declared a code system without a canonical URI."
            </h3>
        }
        .into_any()
    } else {
        // The canonical is the card's own address, so the heading is the link
        // onto the system's screen. The version comes from the shell, which
        // provides it above every screen this card is drawn on.
        let SelectedVersion(version) = expect_context::<SelectedVersion>();
        let target = card.url.clone();
        let href = move || system_link(&target, version.get());
        view! {
            <h3 class="font-mono text-sm font-semibold break-all text-slate-900 dark:text-slate-50">
                <a href=href class="underline decoration-dotted underline-offset-4">
                    {card.url}
                </a>
            </h3>
        }
        .into_any()
    };

    let content = card.content.map_or_else(
        || format!("Content {NOT_DECLARED} at this FHIR version"),
        |mode| format!("Content: {mode}"),
    );
    let subsumption = match card.subsumption {
        Some(true) => "Subsumption supported".to_owned(),
        Some(false) => "Subsumption not supported".to_owned(),
        None => format!("Subsumption {NOT_DECLARED}"),
    };
    let support = view! {
        <ul class="mt-2 flex flex-wrap gap-2 text-xs">
            <li class="rounded bg-slate-100 px-2 py-1 text-slate-800 dark:bg-slate-800 dark:text-slate-100">
                {content}
            </li>
            <li class="rounded bg-slate-100 px-2 py-1 text-slate-800 dark:bg-slate-800 dark:text-slate-100">
                {subsumption}
            </li>
        </ul>
    }
    .into_any();

    let versions = if card.versions.is_empty() {
        view! {
            <p class="mt-3 text-sm text-slate-600 dark:text-slate-300">
                "This server declares no version for this code system."
            </p>
        }
        .into_any()
    } else {
        version_table(&card.versions)
    };

    view! {
        <article class="rounded-lg border border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900">
            {heading} {support} {versions}
        </article>
    }
}

/// The table of served versions, with a real header row and a `<tbody>`.
fn version_table(versions: &[VersionRow]) -> AnyView {
    let rows: Vec<AnyView> = versions.iter().map(version_row).collect();
    view! {
        <div class="mt-3 overflow-x-auto">
            <table class="w-full border-collapse text-left text-sm">
                <caption class="pb-2 text-left text-xs text-slate-600 dark:text-slate-300">
                    "The versions this server holds"
                </caption>
                <thead>
                    <tr class="border-b border-slate-200 dark:border-slate-700">
                        <th scope="col" class="py-1 pr-3 font-medium">
                            "Version"
                        </th>
                        <th scope="col" class="py-1 pr-3 font-medium">
                            "Resolved by default"
                        </th>
                        <th scope="col" class="py-1 pr-3 font-medium">
                            "Compositional grammar"
                        </th>
                        <th scope="col" class="py-1 pr-3 font-medium">
                            "Designation languages"
                        </th>
                        <th scope="col" class="py-1 pr-3 font-medium">
                            "Filters"
                        </th>
                        <th scope="col" class="py-1 font-medium">
                            "Loaded from"
                        </th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
        </div>
    }
    .into_any()
}

/// One served version.
fn version_row(version: &VersionRow) -> AnyView {
    let code = version
        .code
        .clone()
        .unwrap_or_else(|| format!("Version {NOT_DECLARED}"));
    let default = if version.is_default { "yes" } else { "no" };
    let compositional = match version.compositional {
        Some(true) => "yes",
        Some(false) => "no",
        None => NOT_DECLARED,
    };
    let languages = if version.languages.is_empty() {
        "None declared".to_owned()
    } else {
        version.languages.join(", ")
    };
    let filters = filter_list(version);
    let artifact = artifact_cell(version.artifact.as_ref());
    view! {
        <tr class="border-b border-slate-100 align-top last:border-0 dark:border-slate-800">
            <th scope="row" class="py-2 pr-3 font-mono text-xs font-normal break-all">
                {code}
            </th>
            <td class="py-2 pr-3">{default}</td>
            <td class="py-2 pr-3">{compositional}</td>
            <td class="py-2 pr-3">{languages}</td>
            <td class="py-2 pr-3">{filters}</td>
            <td class="py-2">{artifact}</td>
        </tr>
    }
    .into_any()
}

/// The artifact a version was read from, or the statement that it came from
/// none.
///
/// A code system the deployment did not build an index for, such as a registry
/// the server carries or a `CodeSystem` resource posted through the API, comes
/// from no artifact, and the cell says so rather than reading as a gap. Only
/// the artifact's own name is on the wire, so no path is shown.
fn artifact_cell(artifact: Option<&Artifact>) -> AnyView {
    let Some(artifact) = artifact else {
        return view! { <span>"Not loaded from an artifact"</span> }.into_any();
    };
    let name = artifact
        .name
        .clone()
        .unwrap_or_else(|| format!("Artifact name {NOT_DECLARED}"));
    let release = artifact
        .release
        .clone()
        .unwrap_or_else(|| NOT_DECLARED.to_owned());
    view! {
        <span class="font-mono text-xs break-all">{name}</span>
        <span class="block text-xs text-slate-600 dark:text-slate-300">"Release: " {release}</span>
    }
    .into_any()
}

/// The filters a version declares, behind a disclosure so a long list folds.
fn filter_list(version: &VersionRow) -> AnyView {
    if version.filters.is_empty() {
        return view! { <span>"No filter declared"</span> }.into_any();
    }
    let count = version.filters.len();
    let items: Vec<AnyView> = version
        .filters
        .iter()
        .map(|filter| {
            let code = if filter.code.is_empty() {
                format!("Filter {NOT_DECLARED}")
            } else {
                filter.code.clone()
            };
            let operators = if filter.operators.is_empty() {
                "no operator declared".to_owned()
            } else {
                filter.operators.join(", ")
            };
            view! {
                <li>
                    <span class="font-mono">{code}</span>
                    ": "
                    {operators}
                </li>
            }
            .into_any()
        })
        .collect();
    view! {
        <details>
            <summary class="cursor-pointer">
                {if count == 1 { "1 filter".to_owned() } else { format!("{count} filters") }}
            </summary>
            <ul class="mt-1 space-y-1 text-xs">{items}</ul>
        </details>
    }
    .into_any()
}

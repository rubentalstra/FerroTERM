//! The four served FHIR roots, side by side.
//!
//! One process answers `/r4`, `/r4b`, `/r5`, and `/r6`, each with its own
//! generated operation set, so the same request is answered differently per
//! root. This screen reads the four `CapabilityStatement` documents and shows
//! the differences it finds in them. It names no version's behaviour of its
//! own: a change in a vendored package changes what the roots declare, and the
//! screen follows with no edit here.

use leptos::prelude::*;

use crate::comparison::Cell;
use crate::comparison::Comparison;
use crate::comparison::RootAnswer;
use crate::comparison::RootColumn;
use crate::comparison::Row;
use crate::comparison::compare;
use crate::comparison::notes;
use crate::comparison::state_sentence;
use crate::components::NOT_DECLARED;
use crate::components::failure::Failure;
use crate::components::reading::Reading;
use crate::components::request_disclosure::RequestDisclosure;
use crate::fhir::FhirClient;
use crate::fhir::capability::CapabilityStatement;
use crate::fhir::error::FhirError;
use crate::fhir::version::FhirVersion;
use crate::styles;

/// What one root answered, before the screen has read it.
type RootRead = LocalResource<Result<CapabilityStatement, FhirError>>;

/// The classes a table cell shares across both tables.
const CELL: &str = "py-default pr-default align-top";

/// Reads the four roots and draws what they declare, side by side.
///
/// The four reads are issued together: a `LocalResource` begins running its
/// future when it is created (`leptos_server` 0.8.7 `local_resource.rs`, which
/// documents `ArcLocalResource::new` as loading on the client), so creating
/// them in one setup body puts the four requests in flight at once. They are
/// then read inside one `<Reading>`, so the comparison appears complete rather
/// than filling in column by column.
///
/// Nothing on this screen depends on the version switcher: it reads every
/// root, so no read refetches and none goes stale.
pub(crate) fn pane() -> AnyView {
    let client = expect_context::<FhirClient>();
    let roots: [RootRead; 4] = FhirVersion::ALL.map(|version| {
        let client = client.clone();
        LocalResource::new(move || {
            let client = client.clone();
            async move { client.capability_statement(version).await }
        })
    });

    let comparison = comparison_section(roots);
    let requests = requests_section(&client);

    view! {
        <section class="mt-section" aria-labelledby="about-versions-heading">
            <h2 id="about-versions-heading" class=styles::SECTION_TITLE>
                "The four FHIR versions"
            </h2>
            <p class=styles::LEAD>
                "This one server answers R4, R4B, R5, and the R6 ballot from four roots, each with the operation set its own release publishes. Everything below is those four capability statements, read from this browser."
            </p>
            {comparison}
            {requests}
        </section>
    }
    .into_any()
}

/// The four documents, read together and drawn as one comparison.
fn comparison_section(roots: [RootRead; 4]) -> AnyView {
    // The live region is built outside the boundary, so it settles with the
    // read rather than with the fallback it replaces.
    let announcement = Memo::new(move |_| state_sentence(&answers(roots)));

    view! {
        <section class="mt-loose" aria-labelledby="comparison-heading">
            <h3 id="comparison-heading" class=styles::SECTION_TITLE>
                "What each root declares"
            </h3>
            <p aria-live="polite" class=styles::LEAD>
                {announcement}
            </p>
            <Reading label="Reading the four capability statements">
                {move || drawn(&answers(roots))}
            </Reading>
        </section>
    }
    .into_any()
}

/// What the four roots have answered so far, in release order.
fn answers(roots: [RootRead; 4]) -> Vec<(FhirVersion, RootAnswer)> {
    FhirVersion::ALL
        .into_iter()
        .zip(roots)
        .map(|(version, read)| {
            let answer = read
                .with(|answered| {
                    answered
                        .as_ref()
                        .map(|result| RootAnswer::read(result.clone()))
                })
                .unwrap_or(RootAnswer::Unread);
            (version, answer)
        })
        .collect()
}

/// The comparison itself: the two tables, the refusals, and the notes.
fn drawn(answers: &[(FhirVersion, RootAnswer)]) -> AnyView {
    let comparison = compare(answers);
    let refused = refusals_view(answers);
    let facts = table_view(
        "What each root says about itself",
        "Declaration",
        &comparison.columns,
        &comparison.facts,
    );
    let operations = operations_view(&comparison);
    let added = notes_view(answers);
    view! {
        {refused}
        {facts}
        {operations}
        {added}
    }
    .into_any()
}

/// The roots that refused, each in the server's own words.
///
/// A refusal is drawn beside the comparison rather than in place of it, so one
/// root that will not answer never blanks the other three.
fn refusals_view(answers: &[(FhirVersion, RootAnswer)]) -> AnyView {
    let drawn: Vec<AnyView> = answers
        .iter()
        .filter_map(|(version, answer)| {
            let error = answer.failure()?.clone();
            Some(
                view! {
                    <div class="mt-default">
                        <p class="text-body font-medium">{version.label()} " did not answer:"</p>
                        <Failure error=Signal::stored(error) />
                    </div>
                }
                .into_any(),
            )
        })
        .collect();
    if drawn.is_empty() {
        return ().into_any();
    }
    view! { <div>{drawn}</div> }.into_any()
}

/// The operations table, with the sentence that reads its shape.
fn operations_view(comparison: &Comparison) -> AnyView {
    if comparison.operations.is_empty() {
        return view! {
            <p class="mt-loose text-body text-muted">
                "No root that answered declares an operation."
            </p>
        }
        .into_any();
    }
    let table = table_view(
        "The operations each root declares, and at which levels it answers them",
        "Operation",
        &comparison.columns,
        &comparison.operations,
    );
    view! {
        <p class="mt-loose text-body text-muted">
            "A row is one operation. A cell says at which levels that root answers it, which is what its release's own OperationDefinition declares. A row with no resource type in front of the $ is answered on the root itself."
        </p>
        {table}
    }
    .into_any()
}

/// One comparison table: a row heading, then one cell per root.
///
/// The rows are a whole-value replacement with no per-row state, so they are a
/// plain `Vec`, which rebuilds every position when a read settles again. A
/// `<For>` key it retained would be moved rather than re-rendered, and a row
/// would keep the old answer under its new heading.
fn table_view(caption: &str, corner: &str, columns: &[RootColumn], rows: &[Row]) -> AnyView {
    let headers: Vec<AnyView> = columns
        .iter()
        .map(|column| {
            let release = column
                .fhir_version
                .clone()
                .unwrap_or_else(|| NOT_DECLARED.to_owned());
            view! {
                <th scope="col" class="py-default pr-default font-medium">
                    {column.version.label()}
                    <span class="block text-small font-normal text-muted">
                        "FHIR " {release} ", " {column.state}
                    </span>
                </th>
            }
            .into_any()
        })
        .collect();
    let body: Vec<AnyView> = rows.iter().map(row_view).collect();
    view! {
        <div class="mt-default overflow-x-auto">
            <table class="w-full border-collapse text-left text-body">
                <caption class="pb-tight text-left text-small font-medium tracking-wide uppercase">
                    {caption.to_owned()}
                </caption>
                <thead>
                    <tr class="border-b border-line-strong">
                        <th scope="col" class="py-default pr-default font-medium">
                            {corner.to_owned()}
                        </th>
                        {headers}
                    </tr>
                </thead>
                <tbody>{body}</tbody>
            </table>
        </div>
    }
    .into_any()
}

/// One row: what it describes, then what each root says about it.
fn row_view(row: &Row) -> AnyView {
    let cells: Vec<AnyView> = row
        .cells
        .iter()
        .map(|cell| view! { <td class=CELL>{cell_text(cell)}</td> }.into_any())
        .collect();
    view! {
        <tr class="border-b border-line align-top">
            // `wrap-break-word` rather than `break-all`: an operation label
            // is one long token with nowhere to break, but a declaration label
            // is a sentence, and `break-all` chops it mid-word
            // (<https://developer.mozilla.org/en-US/docs/Web/CSS/overflow-wrap>).
            <th
                scope="row"
                class="py-default pr-default font-mono text-small font-normal wrap-break-word"
            >
                {row.label.clone()}
            </th>
            {cells}
        </tr>
    }
    .into_any()
}

/// One cell, in words, because a tint carries no meaning.
fn cell_text(cell: &Cell) -> String {
    match cell {
        Cell::Stated(text) => text.clone(),
        Cell::NotDeclared => NOT_DECLARED.to_owned(),
        Cell::Unread => "no answer".to_owned(),
    }
}

/// What each root says it answers beyond the definition its release publishes.
fn notes_view(answers: &[(FhirVersion, RootAnswer)]) -> AnyView {
    let drawn: Vec<AnyView> = answers
        .iter()
        .map(|(version, answer)| root_notes_view(version.label(), &notes(answer)))
        .collect();
    view! {
        <div class="mt-section">
            <h3 class="text-body font-medium">"What each root adds to its own definition"</h3>
            <p class=styles::LEAD>
                "Each line below is one root's own operation.documentation, as it sent it. It is where a release says which parameters it takes beyond the ones its own OperationDefinition declares."
            </p>
            {drawn}
        </div>
    }
    .into_any()
}

/// One root's notes, closed until a reader opens them.
fn root_notes_view(label: &'static str, notes: &[(String, String)]) -> AnyView {
    let count = notes.len();
    let lines: Vec<AnyView> = notes
        .iter()
        .map(|(operation, note)| {
            view! {
                <dt class="mt-default font-mono text-small wrap-break-word">{operation.clone()}</dt>
                <dd class="text-body text-muted">{note.clone()}</dd>
            }
            .into_any()
        })
        .collect();
    let body = if count == 0 {
        view! {
            <p class="text-body text-muted">
                "This root added nothing to the definitions its release publishes, or did not answer."
            </p>
        }
        .into_any()
    } else {
        view! { <dl>{lines}</dl> }.into_any()
    };
    view! {
        <details class="mt-default rounded border border-line">
            <summary class="cursor-pointer px-default py-default text-body font-medium">
                {label} " · " {note_count(count)}
            </summary>
            <div class="border-t border-line px-default py-default">{body}</div>
        </details>
    }
    .into_any()
}

/// How many operations one root wrote a note on, as a phrase.
fn note_count(count: usize) -> String {
    if count == 1 {
        "1 operation with a note".to_owned()
    } else {
        format!("{count} operations with a note")
    }
}

/// The four requests this screen made, each one a reader can repeat.
fn requests_section(client: &FhirClient) -> AnyView {
    let drawn: Vec<AnyView> = FhirVersion::ALL
        .into_iter()
        .map(|version| {
            let url = client.metadata_url(version);
            view! { <RequestDisclosure url=url label=version.label() /> }.into_any()
        })
        .collect();
    view! { <div class="mt-section">{drawn}</div> }.into_any()
}

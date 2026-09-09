//! The `$translate` runner: one code, through the maps this root holds.
//!
//! The screen reads and never writes. Every parameter of a run lives in the
//! address, so a translation is a link you can share and the back button walks
//! the runs.

use leptos::ev::SubmitEvent;
use leptos::html::Input;
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::NavigateOptions;
use leptos_router::hooks::use_navigate;
use leptos_router::hooks::use_query_map;

use crate::components::NOT_DECLARED;
use crate::components::failure::Failure;
use crate::components::field::Field;
use crate::components::field::Help;
use crate::components::field::group;
use crate::components::field::help_toggle;
use crate::components::field::row;
use crate::components::field::text_field;
use crate::components::icon;
use crate::components::icon::Icon;
use crate::components::reading::Reading;
use crate::components::request_disclosure::RequestDisclosure;
use crate::components::runs::History;
use crate::components::shell::SelectedVersion;
use crate::components::state::undeclared;
use crate::fhir::CONCEPT_MAP;
use crate::fhir::FhirClient;
use crate::fhir::error::FhirError;
use crate::fhir::translate::Coding;
use crate::fhir::translate::NamedValue;
use crate::fhir::translate::TranslateAnswer;
use crate::fhir::translate::TranslateRequest;
use crate::fhir::translate::TranslationMatch;
use crate::fhir::version::FhirVersion;
use crate::routes::TRANSLATE_PATH;
use crate::routes::UI_BASE;
use crate::routes::VERSION_PARAM;
use crate::runs::Run;
use crate::styles;
use crate::url::RequestUrl;

/// The operation this screen runs.
const TRANSLATE: &str = "translate";

/// The address parameter carrying the concept map the runner translates with.
const MAP_PARAM: &str = "map";

/// The address parameter carrying that concept map's business version.
const MAP_VERSION_PARAM: &str = "mapVersion";

/// The address parameter carrying the code system the code belongs to.
const SYSTEM_PARAM: &str = "system";

/// The address parameter carrying that code system's version.
const SYSTEM_VERSION_PARAM: &str = "systemVersion";

/// The address parameter carrying the code to translate.
const CODE_PARAM: &str = "code";

/// The address parameter carrying the code system an answer is wanted in.
const TARGET_PARAM: &str = "target";

/// Runs `ConceptMap/$translate` and renders what the server answered.
///
/// Every parameter is read from the address rather than from a private signal,
/// so a run is shareable by link. The read is reactive: a submit is a
/// navigation onto this same route, and `leptos_router` 0.8.15 then updates
/// the query without re-running this body (`src/nested_router.rs`, the
/// same-route-id branch).
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn TranslatePage() -> impl IntoView {
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    provide_context(Help(RwSignal::new(false)));

    let query = use_query_map();
    let run: Signal<TranslateRequest> =
        Memo::new(move |_| query.with(|map| read_run(&|name| map.get(name)))).into();

    let heading = view! {
        <Title text="Translate" />
        <h1 class=styles::PAGE_TITLE>"Translate a code"</h1>
        <p class=styles::LEAD>"One code through the maps this root holds."</p>
    }
    .into_any();

    // A run this browser remembers is the address that made it, and the
    // address is what the form already puts every parameter in.
    let made = Memo::new(move |_| {
        run.with(|run| {
            run.runnable().then(|| Run {
                screen: "Translate".to_owned(),
                subject: run.code.clone(),
                address: address(run, version.get()),
            })
        })
    });
    let history = History::recording(made);

    let form = runner_section(&client, version, run);
    let answer = answer_section(&client, version, run);

    // The answer keeps its own column above the large breakpoint, so a
    // parameter that lengthens the form never pushes it down the page.
    view! {
        {heading}
        <div class="mt-loose grid items-start gap-loose lg:grid-cols-[24rem_minmax(0,1fr)]">
            <div class="min-w-0">{form} {history.view()}</div>
            <div class="min-w-0">{answer}</div>
        </div>
    }
}

/// The parameters of a run, as the address carries them.
fn read_run(query: &dyn Fn(&str) -> Option<String>) -> TranslateRequest {
    let text = |name: &str| query(name).unwrap_or_default().trim().to_owned();
    TranslateRequest {
        concept_map: text(MAP_PARAM),
        concept_map_version: text(MAP_VERSION_PARAM),
        system: text(SYSTEM_PARAM),
        system_version: text(SYSTEM_VERSION_PARAM),
        code: text(CODE_PARAM),
        target_system: text(TARGET_PARAM),
    }
}

/// A run as the address parameters that reproduce it.
fn run_pairs(run: &TranslateRequest) -> Vec<(&'static str, String)> {
    vec![
        (MAP_PARAM, run.concept_map.clone()),
        (MAP_VERSION_PARAM, run.concept_map_version.clone()),
        (SYSTEM_PARAM, run.system.clone()),
        (SYSTEM_VERSION_PARAM, run.system_version.clone()),
        (CODE_PARAM, run.code.clone()),
        (TARGET_PARAM, run.target_system.clone()),
    ]
}

/// The address one run is, for a link or a navigation.
///
/// A parameter the reader left empty is left out, so the address says what the
/// run sent and nothing more.
pub(crate) fn address(run: &TranslateRequest, version: FhirVersion) -> String {
    let mut url = RequestUrl::new()
        .segment(UI_BASE.trim_start_matches('/'))
        .segment(TRANSLATE_PATH)
        .query(VERSION_PARAM, version.segment());
    for (name, value) in run_pairs(run) {
        if !value.is_empty() {
            url = url.query(name, &value);
        }
    }
    url.render("")
}

/// The navigation options an address that already carries the base needs.
// NOTE: the router resolves a navigation against its base, so an address that
// already carries the base is passed unresolved (`leptos_router` 0.8.15
// `matching/resolve_path.rs`).
fn unresolved() -> NavigateOptions {
    NavigateOptions {
        resolve: false,
        ..NavigateOptions::default()
    }
}

/// What a control holds now, trimmed, or the empty string when it is gone.
fn typed_value(node: NodeRef<Input>) -> String {
    node.get()
        .map(|input| input.value())
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// The `$translate` runner, gated on what this root declares it can do.
///
/// The affordance appears only where the `CapabilityStatement` declares
/// `$translate` on `ConceptMap`, which is where a server states the operations
/// it answers per resource type
/// (<https://hl7.org/fhir/R4B/capabilitystatement.html>), so the screen never
/// offers a run the root then refuses. The answer below it is drawn whenever a
/// run is in the address, because a link a reader was sent still has to say
/// what the server answered.
fn runner_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    run: Signal<TranslateRequest>,
) -> AnyView {
    let read_client = client.clone();
    let statement = LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        async move { client.capability_statement(version).await }
    });
    let url_client = client.clone();
    let url = Signal::derive(move || url_client.metadata_url(version.get()));
    let declared = Signal::derive(move || {
        statement.with(|answered| {
            answered
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .is_some_and(|statement| statement.declares_operation(CONCEPT_MAP, TRANSLATE))
        })
    });
    view! {
        <section class="mt-loose" aria-labelledby="translate-heading">
            <h2 id="translate-heading" class="sr-only">
                "The runner"
            </h2>
            <Reading label="Reading what this root can do">
                {move || {
                    statement
                        .with(|answered| {
                            answered
                                .as_ref()
                                .map(|result| match result {
                                    Ok(statement) => {
                                        if statement.declares_operation(CONCEPT_MAP, TRANSLATE) {
                                            ().into_any()
                                        } else {
                                            undeclared_view()
                                        }
                                    }
                                    Err(error) => failure_view(error),
                                })
                        })
                }}
            </Reading>
            <Show when=move || declared.get() fallback=|| ()>
                {runner_form(run, version)}
            </Show>
            <RequestDisclosure url />
        </section>
    }
    .into_any()
}

/// The statement that this root does not declare the operation.
fn undeclared_view() -> AnyView {
    undeclared(
        "This root's capability statement does not declare $translate on ConceptMap, so the runner is not offered here. Another FHIR version may declare it, so try the version switcher above.",
    )
}

/// A control's own value, gated so an unrelated navigation cannot wipe an edit.
///
/// `prop:value` writes the DOM property on every notification with no equality
/// check (tachys 0.2.18, `html/property.rs`), so a control seeded from a derive
/// over the whole run loses what a reader is typing the moment any other
/// parameter moves. A `Memo` over the one field notifies only when that field
/// changes.
fn seeded(run: Signal<TranslateRequest>, read: fn(&TranslateRequest) -> String) -> Memo<String> {
    Memo::new(move |_| run.with(read))
}

/// The runner, as a form that navigates rather than reloading the page.
fn runner_form(run: Signal<TranslateRequest>, version: Signal<FhirVersion>) -> AnyView {
    let map: NodeRef<Input> = NodeRef::new();
    let map_version: NodeRef<Input> = NodeRef::new();
    let system: NodeRef<Input> = NodeRef::new();
    let system_version: NodeRef<Input> = NodeRef::new();
    let code: NodeRef<Input> = NodeRef::new();
    let target: NodeRef<Input> = NodeRef::new();
    let navigate = use_navigate();
    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        let typed = TranslateRequest {
            concept_map: typed_value(map),
            concept_map_version: typed_value(map_version),
            system: typed_value(system),
            system_version: typed_value(system_version),
            code: typed_value(code),
            target_system: typed_value(target),
        };
        navigate(&address(&typed, version.get()), unresolved());
    };
    view! {
        <form class="mt-loose grid gap-loose" on:submit=submit>
            {map_group(run, map, map_version)}
            {code_group(run, system, system_version, code)}
            {target_group(run, target)}
            <div class="flex flex-wrap items-center gap-default">
                <button type="submit" class=styles::SUBMIT>
                    <Icon glyph=icon::CONCEPT_MAPS />
                    "Translate"
                </button>
                {help_toggle()}
            </div>
        </form>
    }
    .into_any()
}

/// Which map the run reads, and which version of it.
fn map_group(
    run: Signal<TranslateRequest>,
    map: NodeRef<Input>,
    map_version: NodeRef<Input>,
) -> AnyView {
    group(
        "The map",
        vec![row(vec![
            text_field(
                Field {
                    id: "translate-map",
                    name: MAP_PARAM,
                    label: "Concept map canonical",
                    hint: "Sent as the url parameter. Left empty, the server picks the maps it holds for the code.",
                },
                map,
                seeded(run, |run| run.concept_map.clone()),
            ),
            text_field(
                Field {
                    id: "translate-map-version",
                    name: MAP_VERSION_PARAM,
                    label: "Concept map version",
                    hint: "Sent as conceptMapVersion.",
                },
                map_version,
                seeded(run, |run| run.concept_map_version.clone()),
            ),
        ])],
    )
}

/// The code being translated, and the system it is read in.
fn code_group(
    run: Signal<TranslateRequest>,
    system: NodeRef<Input>,
    system_version: NodeRef<Input>,
    code: NodeRef<Input>,
) -> AnyView {
    group(
        "The code",
        vec![
            row(vec![
                text_field(
                    Field {
                        id: "translate-system",
                        name: SYSTEM_PARAM,
                        label: "Code system",
                        hint: "The system the code belongs to. The operation requires one with a code.",
                    },
                    system,
                    seeded(run, |run| run.system.clone()),
                ),
                text_field(
                    Field {
                        id: "translate-system-version",
                        name: SYSTEM_VERSION_PARAM,
                        label: "Code system version",
                        hint: "Left empty, the server resolves the code against its default version.",
                    },
                    system_version,
                    seeded(run, |run| run.system_version.clone()),
                ),
            ]),
            text_field(
                Field {
                    id: "translate-code",
                    name: CODE_PARAM,
                    label: "Code",
                    hint: "The code to translate, sent exactly as you type it. The operation needs this and the system together.",
                },
                code,
                seeded(run, |run| run.code.clone()),
            ),
        ],
    )
}

/// Where the answer is allowed to land.
fn target_group(run: Signal<TranslateRequest>, target: NodeRef<Input>) -> AnyView {
    group(
        "The target",
        vec![text_field(
            Field {
                id: "translate-target",
                name: TARGET_PARAM,
                label: "Target code system",
                hint: "Narrows the answer to matches in this system. Left empty, every target the map reaches is returned.",
            },
            target,
            seeded(run, |run| run.target_system.clone()),
        )],
    )
}

/// The answer of one run: the matches, or the refusal that came instead.
///
/// The resource is created here, once, and the block below reads it. Nothing
/// is drawn until the address carries a code and its system, so a screen that
/// has not been asked to translate anything says nothing about a translation.
fn answer_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    run: Signal<TranslateRequest>,
) -> AnyView {
    let read_client = client.clone();
    let translated = LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        let request = run.get();
        async move {
            if request.runnable() {
                Some(client.translate(version, &request).await)
            } else {
                None
            }
        }
    });
    let url_client = client.clone();
    let url = Signal::derive(move || {
        run.with(|run| {
            if run.runnable() {
                url_client.translate_url(version.get(), run)
            } else {
                String::new()
            }
        })
    });

    // The live region is mounted the moment the address names a run, which is
    // before the read settles, so a screen reader hears the count when it
    // arrives rather than reading a sentence that was already there.
    let announcement = Memo::new(move |_| {
        translated.with(|answered| {
            answered
                .as_ref()
                .and_then(Option::as_ref)
                .and_then(|result| result.as_ref().ok())
                .map(match_sentence)
                .unwrap_or_default()
        })
    });

    view! {
        <Show when=move || run.with(TranslateRequest::runnable) fallback=|| ()>
            {answer_block(translated, announcement, url)}
        </Show>
    }
    .into_any()
}

/// The heading, the live region, the answer, and the request that fetched it.
fn answer_block(
    translated: LocalResource<Option<Result<TranslateAnswer, FhirError>>>,
    announcement: Memo<String>,
    url: Signal<String>,
) -> AnyView {
    view! {
        <section class="mt-loose" aria-labelledby="translate-answer-heading">
            <h2 id="translate-answer-heading" class=styles::SECTION_TITLE>
                "The translation"
            </h2>
            <p aria-live="polite" class="mt-default text-body text-muted">
                {move || announcement.get()}
            </p>
            <Reading label="Running the translation">
                {move || {
                    translated
                        .with(|answered| {
                            answered
                                .as_ref()
                                .map(|answer| match answer {
                                    None => ().into_any(),
                                    Some(Ok(answer)) => answer_view(answer),
                                    Some(Err(error)) => failure_view(error),
                                })
                        })
                }}
            </Reading>
            <RequestDisclosure url />
        </section>
    }
    .into_any()
}

/// The result, the message, and one block per reported match.
fn answer_view(answer: &TranslateAnswer) -> AnyView {
    let result = match answer.result() {
        Some(true) => "The server translated the code.".to_owned(),
        Some(false) => "The server did not translate the code.".to_owned(),
        None => format!("Result {NOT_DECLARED}."),
    };
    let message = answer.message().map_or_else(
        || ().into_any(),
        |message| view! { <p class="mt-tight text-body">{message}</p> }.into_any(),
    );
    let used = used_view(&answer.used_maps());
    let matches = answer.matches();
    if matches.is_empty() {
        return view! {
            <p class="mt-default text-body font-medium">{result}</p>
            {message}
            <p class="mt-default text-body text-muted">
                "The server reported no match for this code."
            </p>
            {used}
        }
        .into_any();
    }
    // The blocks are a plain `Vec`, which rebuilds every position when the
    // next run settles. A `<For>` key it retained would be moved rather than
    // re-rendered, so a target code that repeats across two runs would keep
    // the block it had.
    let blocks: Vec<AnyView> = matches.iter().map(match_view).collect();
    view! {
        <p class="mt-default text-body font-medium">{result}</p>
        {message}
        <div class="mt-default grid gap-loose">{blocks}</div>
        {used}
    }
    .into_any()
}

/// One match: how it relates, what it points at, and what came with it.
fn match_view(found: &TranslationMatch) -> AnyView {
    let target = found.target.as_ref().map_or_else(
        || {
            if found.no_map {
                "This code maps to nothing in the target".to_owned()
            } else {
                format!("Target {NOT_DECLARED}")
            }
        },
        Coding::rendered,
    );
    // The relation is labelled by the element that carried it: R4 and R4B
    // answer `equivalence` and R5 and the R6 ballot `relationship`, over two
    // different sets of codes.
    let relations: Vec<AnyView> = found
        .relations()
        .into_iter()
        .map(|relation| {
            view! {
                <li>
                    <span class="font-medium">{relation.label}</span>
                    ": "
                    <span class="font-mono">{relation.code}</span>
                </li>
            }
            .into_any()
        })
        .collect();
    let stated = if relations.is_empty() {
        view! {
            <p class=styles::LEAD>
                "The server stated no equivalence or relationship for this match."
            </p>
        }
        .into_any()
    } else {
        view! { <ul class="mt-tight text-body">{relations}</ul> }.into_any()
    };
    let facts: Vec<AnyView> = [
        (
            "From concept",
            found.source_concept.as_ref().map(Coding::rendered),
        ),
        ("From map", found.origin_map.clone()),
        ("Source comment", found.source_comment.clone()),
        ("Target comment", found.target_comment.clone()),
    ]
    .into_iter()
    .filter_map(|(label, value)| value.map(|value| (label, value)))
    .map(|(label, value)| {
        view! {
            <div class="grid gap-tight sm:grid-cols-[10rem_1fr]">
                <dt class="font-medium">{label}</dt>
                <dd class="font-mono break-all">{value}</dd>
            </div>
        }
        .into_any()
    })
    .collect();
    let products = values_view("Also produces", &found.products);
    let depends = values_view("Depends on", &found.depends_on);
    let properties = values_view("Properties", &found.properties);
    view! {
        <article class=format!("panel-p {}", styles::PANEL)>
            <h4 class="font-mono text-body font-semibold break-all">{target}</h4>
            {stated}
            <dl class="mt-default text-small">{facts}</dl>
            {products}
            {depends}
            {properties}
        </article>
    }
    .into_any()
}

/// One list of named values a match carried, or nothing when it carried none.
fn values_view(label: &'static str, values: &[NamedValue]) -> AnyView {
    if values.is_empty() {
        return ().into_any();
    }
    let items: Vec<AnyView> = values
        .iter()
        .map(|value| {
            let rendered = value
                .value
                .clone()
                .unwrap_or_else(|| "a value shape this viewer draws no text for".to_owned());
            view! {
                <li class="break-all">
                    <span class="font-mono">{value.name.clone()}</span>
                    ": "
                    {rendered}
                </li>
            }
            .into_any()
        })
        .collect();
    view! {
        <h5 class="mt-default text-small font-medium tracking-wide uppercase">{label}</h5>
        <ul class="mt-tight ml-loose list-disc text-small">{items}</ul>
    }
    .into_any()
}

/// The maps the server says it translated through.
fn used_view(used: &[String]) -> AnyView {
    if used.is_empty() {
        return ().into_any();
    }
    let items: Vec<AnyView> = used
        .iter()
        .map(|canonical| view! { <li class="break-all">{canonical.clone()}</li> }.into_any())
        .collect();
    view! {
        <h4 class="mt-loose text-small font-medium tracking-wide uppercase">
            "The maps the server used"
        </h4>
        <ul class="mt-tight ml-loose list-disc font-mono text-small">{items}</ul>
    }
    .into_any()
}

/// A refused read, in the server's own words.
fn failure_view(error: &FhirError) -> AnyView {
    let error = error.clone();
    view! {
        <div class="mt-default">
            <Failure error=Signal::stored(error) />
        </div>
    }
    .into_any()
}

/// How many matches a translation reported, as a sentence.
fn match_sentence(answer: &TranslateAnswer) -> String {
    match answer.matches().len() {
        0 => "No matches.".to_owned(),
        1 => "1 match.".to_owned(),
        matches => format!("{matches} matches."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for the address, which reads the same way a `ParamsMap` does.
    fn query<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + use<'a> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn a_run_reads_out_of_the_address_and_back_into_it() {
        let run = read_run(&query(&[
            ("map", "https://terminology.example/ConceptMap/m"),
            ("system", "https://terminology.example/a"),
            ("code", "x"),
        ]));
        assert!(run.runnable());
        assert_eq!(
            address(&run, FhirVersion::R4B),
            "/ui/translate?fhir=r4b&map=https%3A%2F%2Fterminology.example%2FConceptMap%2Fm\
             &system=https%3A%2F%2Fterminology.example%2Fa&code=x",
            "a parameter the reader left empty is left out of the address"
        );
    }

    #[test]
    fn a_run_naming_no_code_is_not_sent() {
        let run = read_run(&query(&[("system", "https://x.example/a")]));
        assert!(
            !run.runnable(),
            "the operation requires a code with its system"
        );
    }

    #[test]
    fn the_match_count_is_the_number_of_matches_the_server_reported() {
        let answer: TranslateAnswer = serde_json::from_str(
            r#"{"parameter":[{"name":"result","valueBoolean":true},
                {"name":"match","part":[{"name":"concept","valueCoding":{"code":"a"}}]},
                {"name":"match","part":[{"name":"concept","valueCoding":{"code":"b"}}]}]}"#,
        )
        .expect("the fixture is valid JSON");
        assert_eq!(match_sentence(&answer), "2 matches.");
    }
}

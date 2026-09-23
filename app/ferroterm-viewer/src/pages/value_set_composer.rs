//! The value set composer: build a `ValueSet.compose`, preview it, save it.
//!
//! The screen is the editor bundle's own. It draws the composition rules FHIR
//! defines as they apply (<https://hl7.org/fhir/R4B/valueset.html#compositions>),
//! previews the unsaved definition with `$expand` by `POST`
//! (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>), and writes
//! through the RESTful API with `If-Match`
//! (<https://hl7.org/fhir/R4B/http.html#concurrency>).
//!
//! Nothing here names a code system. The systems come from the served root's
//! `TerminologyCapabilities`, the value sets from `GET [base]/ValueSet`, and
//! the implicit forms from the filters the capability statement declares.

use leptos::ev::Event;
use leptos::ev::SubmitEvent;
use leptos::html::Input;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_meta::Title;
use leptos_router::NavigateOptions;
use leptos_router::hooks::use_navigate;
use leptos_router::hooks::use_query_map;

use crate::auth::Session;
use crate::auth::scopes::Letter;
use crate::components::failure::Failure;
use crate::components::field::Field;
use crate::components::field::Help;
use crate::components::field::group;
use crate::components::field::help_toggle;
use crate::components::icon;
use crate::components::icon::Icon;
use crate::components::reading::Reading;
use crate::components::shell::SelectedVersion;
use crate::fhir::FhirClient;
use crate::fhir::VALUE_SET;
use crate::fhir::compose::Broken;
use crate::fhir::compose::Clause;
use crate::fhir::compose::Draft;
use crate::fhir::compose::Preview;
use crate::fhir::compose::STATUSES;
use crate::fhir::compose::StoredValueSet;
use crate::fhir::concept::ConceptQuery;
use crate::fhir::error::FhirError;
use crate::fhir::expansion::ConceptRow;
use crate::fhir::expansion::ExpandedValueSet;
use crate::fhir::implicit;
use crate::fhir::named::Choice;
use crate::fhir::outcome::OperationOutcome;
use crate::fhir::terminology::FilterRow;
use crate::fhir::terminology::TerminologyCapabilities;
use crate::fhir::version::FhirVersion;
use crate::fhir::write::Refusal;
use crate::offers::choices;
use crate::offers::published;
use crate::paging::MAX_COUNT;
use crate::paging::Page;
use crate::routes::COMPOSE_PATH;
use crate::routes::VERSION_PARAM;
use crate::routes::base_url;
use crate::styles;

/// The query parameter naming the value set being composed.
const ID_PARAM: &str = "id";

/// The query parameter carrying the preview's text filter.
const FILTER_PARAM: &str = "filter";

/// The query parameter carrying the preview's page size.
const COUNT_PARAM: &str = "count";

/// The query parameter carrying the preview's offset.
const OFFSET_PARAM: &str = "offset";

/// The page the preview opens on when the address names none.
const DEFAULT_COUNT: u32 = 20;

/// How many concepts one code search answers.
const SEARCH_COUNT: u32 = 20;

/// The live region every refusal and every count is announced in.
const REPORT_ID: &str = "composer-report";

/// Composes a local `ValueSet`, previews it, and saves it.
///
/// The composition itself is a document being edited rather than a filter, so
/// it lives in a signal; the preview's page and text filter are the address,
/// which is what makes a previewed page shareable and the back button work
/// (<https://github.com/leptos-rs/book/blob/main/src/router/20_form.md>).
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn ValueSetComposerPage() -> impl IntoView {
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let session = expect_context::<Session>();

    let query = use_query_map();
    let id: Memo<String> = Memo::new(move |_| {
        query.with(|map| map.get(ID_PARAM).unwrap_or_default().trim().to_owned())
    });
    let page: Memo<Preview> = Memo::new(move |_| {
        query.with(|map| Preview {
            filter: map
                .get(FILTER_PARAM)
                .map(|typed| typed.trim().to_owned())
                .filter(|typed| !typed.is_empty()),
            count: map
                .get(COUNT_PARAM)
                .and_then(|typed| typed.trim().parse::<u32>().ok())
                .unwrap_or(DEFAULT_COUNT)
                .clamp(1, MAX_COUNT),
            offset: map
                .get(OFFSET_PARAM)
                .and_then(|typed| typed.trim().parse::<u32>().ok())
                .unwrap_or_default(),
        })
    });

    // The resource is read again after a save, so the version an `If-Match`
    // states is the one the server just committed. The counter is written from
    // the save handler, which is an event rather than an effect.
    let saves = RwSignal::new(0_u32);
    let answered = reading(&client, version, id, saves);
    let editing = Editing {
        edited: RwSignal::new(None),
        stored: Memo::new(move |_| {
            answered.with(|held| held.as_ref().map_or_else(Draft::new, StoredValueSet::draft))
        }),
    };
    let writable = writable(id, answered, session);

    let report = RwSignal::new(String::new());
    let refusal: RwSignal<Option<FhirError>> = RwSignal::new(None);
    let previewed: RwSignal<Option<Draft>> = RwSignal::new(None);
    let picking: RwSignal<Option<u32>> = RwSignal::new(None);
    let term = RwSignal::new(String::new());

    let offers = Offers {
        value_sets: choices(published(&client, version, VALUE_SET)),
        capabilities: capabilities(&client, version),
        picking,
        term,
    };

    provide_context(Help(RwSignal::new(false)));

    let expansion = previewing(&client, version, previewed, page);
    let announcement = announcing(expansion);
    // One live region carries every message the screen has, so a screen reader
    // hears the count, the refusal, and the save in one place
    // (<https://www.w3.org/TR/wai-aria-1.2/#aria-live>). What an event wrote
    // wins until the next preview is asked for, which clears it.
    let said = move || {
        let written = report.get();
        if written.is_empty() {
            announcement.get()
        } else {
            written
        }
    };

    let heading = view! {
        <Title text="Compose a value set" />
        <h1 class=styles::PAGE_TITLE>"Compose a value set"</h1>
        <p class=styles::LEAD>
            "Draw in a published value set, add your own codes, and see what the selection holds."
        </p>
        <p id=REPORT_ID aria-live="polite" class=format!("mt-default {}", styles::MUTED)>
            {said}
        </p>
    }
    .into_any();

    let banner = read_only_banner(writable, id, session);
    let identity = identity_section(editing, writable);
    let clauses = clause_sections(editing, writable, offers);
    let actions = save_section(
        &client, version, editing, writable, session, report, refusal, saves, previewed,
    );
    let refused = refusal_section(refusal, editing);
    let preview = preview_section(expansion, previewed, page, editing, report);

    view! {
        {heading}
        <div class="mt-loose grid items-start gap-loose lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
            <div class="min-w-0">{banner} {identity} {clauses} {actions} {refused}</div>
            <div class="min-w-0">{preview}</div>
        </div>
    }
}

/// Reads the `ValueSet` the address names, again after every save.
fn reading(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    id: Memo<String>,
    saves: RwSignal<u32>,
) -> Memo<Option<StoredValueSet>> {
    let client = client.clone();
    let stored = LocalResource::new(move || {
        let client = client.clone();
        let version = version.get();
        let id = id.get();
        let _after_a_save = saves.get();
        async move {
            if id.is_empty() {
                None
            } else {
                Some(client.value_set_stored(version, &id).await)
            }
        }
    });
    Memo::new(move |_| {
        stored.with(|read| {
            read.as_ref()
                .and_then(Option::as_ref)
                .and_then(|result| result.as_ref().ok())
                .cloned()
        })
    })
}

/// Whether the screen may offer to save what it is showing.
///
/// National content has no write path: the built indexes open read-only, and
/// the server stamps `meta.versionId` only on what it holds a writable record
/// of (<https://hl7.org/fhir/R4B/http.html#concurrency>), so a resource
/// carrying none is read even for a token that holds every scope.
fn writable(
    id: Memo<String>,
    answered: Memo<Option<StoredValueSet>>,
    session: Session,
) -> Memo<bool> {
    Memo::new(move |_| {
        let new_draft = id.with(String::is_empty);
        let letter = if new_draft {
            Letter::Create
        } else {
            Letter::Update
        };
        session.can(VALUE_SET, letter)
            && (new_draft
                || answered.with(|held| held.as_ref().is_some_and(StoredValueSet::writable)))
    })
}

/// The draft on screen: the reader's edits, or the resource as it was read.
///
/// Holding the edits beside the read rather than copying the read into a
/// signal is what keeps an effect out of this screen: nothing writes a signal
/// from another signal's value, and reopening the resource is dropping the
/// edits (`working_with_signals` §4).
#[derive(Clone, Copy)]
struct Editing {
    /// What the reader has changed, absent until they change something.
    edited: RwSignal<Option<Draft>>,
    /// The resource as the server answered it, or a new draft.
    stored: Memo<Draft>,
}

impl Editing {
    /// One value of the draft on screen, as its own memo.
    fn part<T>(self, read: impl Fn(&Draft) -> T + Send + Sync + 'static) -> Memo<T>
    where
        T: PartialEq + Send + Sync + 'static,
    {
        Memo::new(move |_| {
            self.edited
                .with(|held| held.as_ref().map(&read))
                .unwrap_or_else(|| self.stored.with(&read))
        })
    }

    /// The draft on screen, cloned, for a request that carries the whole of it.
    fn draft(self) -> Draft {
        self.edited
            .with(Clone::clone)
            .unwrap_or_else(|| self.stored.get())
    }

    /// Applies one change to the draft on screen.
    fn change(self, apply: impl FnOnce(&mut Draft)) {
        let mut draft = self
            .edited
            .with_untracked(Clone::clone)
            .unwrap_or_else(|| self.stored.get_untracked());
        apply(&mut draft);
        self.edited.set(Some(draft));
    }

    /// Drops the edits, so the screen shows the resource as the server holds it.
    fn reopen(self) {
        self.edited.set(None);
    }
}

/// What the pickers on the form offer, read once for the whole screen.
#[derive(Clone, Copy)]
struct Offers {
    /// The value sets this root publishes.
    value_sets: Memo<Vec<Choice>>,
    /// What the root declares about the systems it serves.
    capabilities: Memo<TerminologyCapabilities>,
    /// The clause whose code search is open, when one is.
    picking: RwSignal<Option<u32>>,
    /// What that search is looking for.
    term: RwSignal<String>,
}

impl Offers {
    /// The code systems this root serves, as a picker's offers.
    fn systems(self) -> Vec<Choice> {
        self.capabilities.with(|declared| {
            declared
                .cards()
                .into_iter()
                .filter(|card| !card.url.is_empty())
                .map(|card| Choice {
                    canonical: card.url.clone(),
                    label: card.url,
                })
                .collect()
        })
    }

    /// The filters the served version declares for `system`.
    ///
    /// A system serving several versions declares filters per version, and the
    /// default version is the one an unversioned clause is answered against
    /// (<https://hl7.org/fhir/R5/terminology-module.html#version>).
    fn filters(self, system: &str) -> Vec<FilterRow> {
        self.capabilities.with(|declared| {
            declared
                .card(system)
                .map(|card| {
                    let default = card
                        .versions
                        .iter()
                        .find(|version| version.is_default)
                        .or_else(|| card.versions.first());
                    default
                        .map(|version| version.filters.clone())
                        .unwrap_or_default()
                })
                .unwrap_or_default()
        })
    }
}

/// What the root declares about the systems it serves.
fn capabilities(
    client: &FhirClient,
    version: Signal<FhirVersion>,
) -> Memo<TerminologyCapabilities> {
    let client = client.clone();
    let read = LocalResource::new(move || {
        let client = client.clone();
        let version = version.get();
        async move { client.terminology_capabilities(version).await }
    });
    Memo::new(move |_| {
        read.with(|answered| {
            answered
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .cloned()
                .unwrap_or_default()
        })
    })
}

/// The notice a screen that cannot be saved carries.
fn read_only_banner(writable: Memo<bool>, id: Memo<String>, session: Session) -> AnyView {
    let sentence = move || {
        if session.signed_in() {
            if id.with(String::is_empty) {
                "This account carries no permission to create a value set, so the form is read-only."
            } else {
                "This value set has no write path on this server, or this account carries no permission to change it. The form is read-only."
            }
        } else {
            "Sign in to save. Everything below composes and previews without a sign-in."
        }
    };
    view! {
        <Show when=move || !writable.get() fallback=|| ()>
            <p class=format!("mt-default rounded-md panel-p {}", styles::NOTICE)>{sentence}</p>
        </Show>
    }
    .into_any()
}

/// One labelled control over a value of the draft.
fn edit_field(field: Field, value: Memo<String>, change: impl FnMut(Event) + 'static) -> AnyView {
    let described_by = format!("{}-note", field.id);
    view! {
        <div class="grid gap-tight">
            <label for=field.id class=styles::LABEL>
                {field.label}
            </label>
            <input
                id=field.id
                name=field.name
                type="text"
                class=styles::INPUT
                aria-describedby=described_by.clone()
                prop:value=move || value.get()
                on:input=change
            />
            <p id=described_by class=styles::HINT>
                {field.hint}
            </p>
        </div>
    }
    .into_any()
}

/// The value set's own identity, and the one flag `compose` carries.
fn identity_section(editing: Editing, writable: Memo<bool>) -> AnyView {
    let url = editing.part(|draft| draft.url.clone());
    let name = editing.part(|draft| draft.name.clone());
    let title = editing.part(|draft| draft.title.clone());
    let fields = vec![
        edit_field(
            Field {
                id: "compose-url",
                name: "url",
                label: "Canonical",
                hint: "ValueSet.url, what everything else refers to this value set by.",
            },
            url,
            move |event| {
                let typed = event_target_value(&event);
                editing.change(move |draft| draft.url = typed);
            },
        ),
        edit_field(
            Field {
                id: "compose-name",
                name: "name",
                label: "Name",
                hint: "ValueSet.name, the computer-friendly name.",
            },
            name,
            move |event| {
                let typed = event_target_value(&event);
                editing.change(move |draft| draft.name = typed);
            },
        ),
        edit_field(
            Field {
                id: "compose-title",
                name: "title",
                label: "Title",
                hint: "ValueSet.title, the name written for a person.",
            },
            title,
            move |event| {
                let typed = event_target_value(&event);
                editing.change(move |draft| draft.title = typed);
            },
        ),
        status_control(editing),
        inactive_control(editing),
    ];

    view! {
        <fieldset class="mt-loose grid gap-default" disabled=move || !writable.get()>
            <legend class=format!("{} mb-tight", styles::EYEBROW)>"The value set"</legend>
            <div class="grid gap-default sm:grid-cols-2">{fields}</div>
        </fieldset>
    }
    .into_any()
}

/// The publication status the resource carries.
fn status_control(editing: Editing) -> AnyView {
    let status = editing.part(|draft| draft.status.clone());
    let statuses: Vec<AnyView> = STATUSES
        .into_iter()
        .map(|code| view! { <option value=code>{code}</option> }.into_any())
        .collect();
    view! {
        <div class="grid gap-tight">
            <label for="compose-status" class=styles::LABEL>
                "Status"
            </label>
            <select
                id="compose-status"
                name="status"
                class=styles::INPUT
                aria-describedby="compose-status-note"
                prop:value=move || status.get()
                on:change=move |event| {
                    let chosen = event_target_value(&event);
                    editing.change(move |draft| draft.status = chosen);
                }
            >
                {statuses}
            </select>
            <p id="compose-status-note" class=styles::HINT>
                "ValueSet.status, which every definitional resource carries."
            </p>
        </div>
    }
    .into_any()
}

/// Whether inactive codes are in the selection.
///
/// It has three states: "if inactive = true, inactive codes are to be included
/// in the expansion, if inactive = false, the inactive codes will not be
/// included … If absent, the behavior is determined by the implementation"
/// (<https://hl7.org/fhir/R4B/valueset.html>).
fn inactive_control(editing: Editing) -> AnyView {
    let inactive = editing.part(|draft| draft.inactive);
    view! {
        <div class="grid gap-tight">
            <label for="compose-inactive" class=styles::LABEL>
                "Inactive codes"
            </label>
            <select
                id="compose-inactive"
                name="inactive"
                class=styles::INPUT
                aria-describedby="compose-inactive-note"
                prop:value=move || match inactive.get() {
                    Some(true) => "true",
                    Some(false) => "false",
                    None => "",
                }
                on:change=move |event| {
                    let chosen = match event_target_value(&event).as_str() {
                        "true" => Some(true),
                        "false" => Some(false),
                        _unset => None,
                    };
                    editing.change(move |draft| draft.inactive = chosen);
                }
            >
                <option value="">"Leave it to the server"</option>
                <option value="true">"Include them"</option>
                <option value="false">"Leave them out"</option>
            </select>
            <p id="compose-inactive-note" class=styles::HINT>
                "compose.inactive. Left to the server, the expansion parameters decide."
            </p>
        </div>
    }
    .into_any()
}

/// The includes and the excludes, each as its own labelled section.
fn clause_sections(editing: Editing, writable: Memo<bool>, offers: Offers) -> AnyView {
    let includes = clause_section(editing, writable, offers, true);
    let excludes = clause_section(editing, writable, offers, false);
    view! {
        {includes}
        {excludes}
    }
    .into_any()
}

/// One side of the compose: every include, or every exclude.
fn clause_section(
    editing: Editing,
    writable: Memo<bool>,
    offers: Offers,
    included: bool,
) -> AnyView {
    let keys: Memo<Vec<u32>> = editing.part(move |draft| draft.clause_keys(included));
    let heading = if included { "Includes" } else { "Excludes" };
    let heading_id = if included {
        "compose-includes-heading"
    } else {
        "compose-excludes-heading"
    };
    let rule = if included {
        "Every include adds to the selection: the value set is the union of them all."
    } else {
        "Every exclude takes codes back out of that union, whichever include drew them in."
    };
    let cards = view! {
        <For
            each=move || keys.get()
            key=|key| *key
            children=move |key| clause_card(editing, writable, offers, key)
        />
    }
    .into_any();
    view! {
        <section class="mt-section" aria-labelledby=heading_id>
            <h2 id=heading_id class=styles::SECTION_TITLE>
                {heading}
            </h2>
            <p class=styles::LEAD>{rule}</p>
            <div class="mt-default grid gap-default">{cards}</div>
            <button
                type="button"
                class=format!("mt-default {}", styles::BUTTON)
                disabled=move || !writable.get()
                on:click=move |_| {
                    editing
                        .change(|draft| {
                            draft.add_clause(included);
                        });
                }
            >
                {if included { "Add an include" } else { "Add an exclude" }}
            </button>
        </section>
    }
    .into_any()
}

/// One clause: what it selects, and the rule that says how.
fn clause_card(editing: Editing, writable: Memo<bool>, offers: Offers, key: u32) -> AnyView {
    let clause: Memo<Clause> =
        editing.part(move |draft| draft.clause(key).cloned().unwrap_or_default());
    let rule = Memo::new(move |_| clause.with(Clause::rule));
    let defect: Memo<Option<Broken>> =
        editing.part(move |draft| draft.broken().into_iter().find(|broken| broken.key == key));
    let system = Memo::new(move |_| clause.with(|clause| clause.system.clone()));

    let value_sets = value_set_rows(editing, offers, key, clause);
    let codes = concept_rows(editing, offers, key, clause);
    let filters = filter_rows(editing, offers, key, clause, system);
    let system_control = system_picker(editing, offers, key, system, clause);

    view! {
        <fieldset
            class=format!("grid gap-default panel-p {}", styles::PANEL)
            disabled=move || !writable.get()
        >
            <legend class=styles::EYEBROW>
                {move || {
                    if clause.with(|clause| clause.included) { "Include" } else { "Exclude" }
                }}
            </legend>
            <p class=styles::MUTED>{move || rule.get()}</p>
            <Show when=move || defect.get().is_some() fallback=|| ()>
                <p role="status" class=format!("rounded-md panel-p {}", styles::NOTICE)>
                    {move || {
                        defect
                            .get()
                            .map(|broken| {
                                format!(
                                    "{}: {}",
                                    broken.defect.constraint(),
                                    broken.defect.sentence(),
                                )
                            })
                            .unwrap_or_default()
                    }}
                </p>
            </Show>
            {value_sets}
            {system_control}
            {codes}
            {filters}
            <button
                type="button"
                class=styles::BUTTON_QUIET
                on:click=move |_| editing.change(move |draft| draft.remove_clause(key))
            >
                "Remove this clause"
            </button>
        </fieldset>
    }
    .into_any()
}

/// The value sets one clause draws in whole, and the two ways to add one.
fn value_set_rows(editing: Editing, offers: Offers, key: u32, clause: Memo<Clause>) -> AnyView {
    let rows: Memo<Vec<u32>> = Memo::new(move |_| {
        clause.with(|clause| clause.value_sets.iter().map(|row| row.key).collect())
    });
    let listed = view! {
        <For
            each=move || rows.get()
            key=|row| *row
            children=move |row| value_set_row(editing, key, row, clause)
        />
    }
    .into_any();
    let adders = add_value_set(editing, offers, key);
    group(
        "Value sets",
        vec![
            view! { <div class="grid gap-tight">{listed}</div> }.into_any(),
            adders,
        ],
    )
}

/// One value set reference, with its implicit form opened where it is in one.
fn value_set_row(editing: Editing, key: u32, row: u32, clause: Memo<Clause>) -> AnyView {
    let canonical = Memo::new(move |_| {
        clause.with(|clause| {
            clause
                .value_sets
                .iter()
                .find(|held| held.key == row)
                .map(|held| held.canonical.clone())
                .unwrap_or_default()
        })
    });
    let implicit = Memo::new(move |_| canonical.with(|canonical| implicit::read(canonical)));
    let control_id = format!("compose-valueset-{key}-{row}");
    let described_by = format!("{control_id}-note");
    let label = move || {
        implicit.get().map_or_else(
            || String::from("Canonical"),
            |(_, form, _)| form.value_label.to_owned(),
        )
    };
    let shown = move || {
        implicit
            .get()
            .map_or_else(|| canonical.get(), |(_, _, value)| value)
    };
    let note = move || {
        implicit.get().map_or_else(
            || canonical.get(),
            |(system, form, _)| format!("{} of {system}", form.label),
        )
    };
    view! {
        <div class="grid gap-tight">
            <label for=control_id.clone() class=styles::LABEL>
                {label}
            </label>
            <div class="flex items-center gap-default">
                <input
                    id=control_id
                    name="valueSet"
                    type="text"
                    class=styles::INPUT
                    aria-describedby=described_by.clone()
                    prop:value=shown
                    on:input=move |event| {
                        let typed = event_target_value(&event);
                        let rewritten = implicit
                            .get_untracked()
                            .map_or_else(
                                || typed.clone(),
                                |(system, form, _)| form.canonical(&system, &typed),
                            );
                        editing
                            .change(move |draft| {
                                if let Some(clause) = draft.clause_mut(key)
                                    && let Some(held) = clause
                                        .value_sets
                                        .iter_mut()
                                        .find(|held| held.key == row)
                                {
                                    held.canonical = rewritten;
                                }
                            });
                    }
                />
                <button
                    type="button"
                    class=styles::BUTTON_QUIET
                    on:click=move |_| {
                        editing
                            .change(move |draft| {
                                if let Some(clause) = draft.clause_mut(key) {
                                    clause.value_sets.retain(|held| held.key != row);
                                }
                            });
                    }
                >
                    "Remove"
                </button>
            </div>
            <p id=described_by class=format!("{} wrap-break-word", styles::HINT)>
                {note}
            </p>
        </div>
    }
    .into_any()
}

/// The control that adds a value set: one this root publishes, or an implicit
/// form of a system it serves.
fn add_value_set(editing: Editing, offers: Offers, key: u32) -> AnyView {
    let published = add_published_value_set(editing, offers, key);
    let implicit_form = add_implicit_value_set(editing, offers, key);
    view! { <div class="grid gap-default">{published} {implicit_form}</div> }.into_any()
}

/// The control that adds one of the value sets this root publishes.
fn add_published_value_set(editing: Editing, offers: Offers, key: u32) -> AnyView {
    let picked = RwSignal::new(String::new());
    let published_options = move || {
        offers.value_sets.with(|choices| {
            choices
                .iter()
                .map(|choice| {
                    view! {
                        <option value=choice.canonical.clone() title=choice.canonical.clone()>
                            {choice.label.clone()}
                        </option>
                    }
                    .into_any()
                })
                .collect::<Vec<AnyView>>()
        })
    };
    view! {
        <div class="grid gap-tight">
            <label for=format!("compose-add-vs-{key}") class=styles::LABEL>
                "Add a published value set"
            </label>
            <div class="flex items-center gap-default">
                <select
                    id=format!("compose-add-vs-{key}")
                    class=styles::INPUT
                    prop:value=move || picked.get()
                    on:change=move |event| picked.set(event_target_value(&event))
                >
                    <option value="">"Choose one this server holds"</option>
                    {published_options}
                </select>
                <button
                    type="button"
                    class=styles::BUTTON
                    on:click=move |_| {
                        let canonical = picked.get_untracked();
                        if !canonical.is_empty() {
                            editing.change(move |draft| draft.add_value_set(key, &canonical));
                            picked.set(String::new());
                        }
                    }
                >
                    "Add"
                </button>
            </div>
        </div>
    }
    .into_any()
}

/// The control that adds a value set a code system defines for itself.
///
/// A form appears only where the selected system's served version declares the
/// filter property and operator the form is shorthand for, so this offers what
/// the capability statement says the server can answer.
fn add_implicit_value_set(editing: Editing, offers: Offers, key: u32) -> AnyView {
    let system = RwSignal::new(String::new());
    let keyword = RwSignal::new(String::new());
    let system_options = move || {
        offers
            .systems()
            .into_iter()
            .map(|choice| {
                view! { <option value=choice.canonical.clone()>{choice.label}</option> }.into_any()
            })
            .collect::<Vec<AnyView>>()
    };
    let forms = Memo::new(move |_| {
        let named = system.get();
        if named.is_empty() {
            Vec::new()
        } else {
            implicit::offered(&offers.filters(&named))
        }
    });
    let form_options = move || {
        forms
            .get()
            .into_iter()
            .map(|form| view! { <option value=form.keyword>{form.label}</option> }.into_any())
            .collect::<Vec<AnyView>>()
    };
    view! {
        <div class="grid gap-tight">
            <label for=format!("compose-add-implicit-system-{key}") class=styles::LABEL>
                "Add a value set a code system defines for itself"
            </label>
            <div class="grid gap-default sm:grid-cols-2">
                <select
                    id=format!("compose-add-implicit-system-{key}")
                    class=styles::INPUT
                    prop:value=move || system.get()
                    on:change=move |event| {
                        system.set(event_target_value(&event));
                        keyword.set(String::new());
                    }
                >
                    <option value="">"Choose a code system"</option>
                    {system_options}
                </select>
                <select
                    id=format!("compose-add-implicit-form-{key}")
                    class=styles::INPUT
                    aria-label="The form the code system defines"
                    prop:value=move || keyword.get()
                    on:change=move |event| keyword.set(event_target_value(&event))
                >
                    <option value="">"Choose a form"</option>
                    {form_options}
                </select>
            </div>
            <Show
                when=move || system.with(|named| !named.is_empty()) && forms.with(Vec::is_empty)
                fallback=|| ()
            >
                <p class=styles::HINT>
                    "This server declares no filter for that system that one of these forms is shorthand for."
                </p>
            </Show>
            <button
                type="button"
                class=styles::BUTTON
                on:click=move |_| {
                    let named = system.get_untracked();
                    let Some(form) = implicit::form(&keyword.get_untracked()) else {
                        return;
                    };
                    let canonical = form.canonical(&named, "");
                    editing.change(move |draft| draft.add_value_set(key, &canonical));
                    keyword.set(String::new());
                }
            >
                "Add the form"
            </button>
        </div>
    }
    .into_any()
}

/// The code system one clause reads codes from.
///
/// Every control carries the clause's key in its `id`, because a screen draws
/// as many of these as the compose has clauses and a label points at one
/// control (<https://www.w3.org/TR/wai-aria-1.2/#namecalculation>).
fn system_picker(
    editing: Editing,
    offers: Offers,
    key: u32,
    system: Memo<String>,
    clause: Memo<Clause>,
) -> AnyView {
    let system_version = Memo::new(move |_| clause.with(|clause| clause.system_version.clone()));
    let served = move || {
        offers
            .systems()
            .into_iter()
            .map(|choice| {
                view! { <option value=choice.canonical.clone()>{choice.label}</option> }.into_any()
            })
            .collect::<Vec<AnyView>>()
    };
    let select_id = format!("compose-system-{key}");
    let typed_id = format!("compose-system-typed-{key}");
    let version_id = format!("compose-system-version-{key}");
    let write_system = move |value: String| {
        editing.change(move |draft| {
            if let Some(clause) = draft.clause_mut(key) {
                clause.system = value;
            }
        });
    };
    view! {
        <div class="grid gap-default sm:grid-cols-2">
            <div class="grid gap-tight">
                <label for=select_id.clone() class=styles::LABEL>
                    "Code system"
                </label>
                <select
                    id=select_id
                    class=styles::INPUT
                    aria-describedby=format!("compose-system-note-{key}")
                    prop:value=move || system.get()
                    on:change=move |event| write_system(event_target_value(&event))
                >
                    <option value="">"Choose one this server serves"</option>
                    {served}
                </select>
                <p id=format!("compose-system-note-{key}") class=styles::HINT>
                    "compose.include.system, the code system the codes below are read from."
                </p>
            </div>
            <div class="grid gap-tight">
                <label for=typed_id.clone() class=styles::LABEL>
                    "Or a canonical this server does not serve"
                </label>
                <input
                    id=typed_id
                    name="system"
                    type="text"
                    class=styles::INPUT
                    prop:value=move || system.get()
                    on:input=move |event| write_system(event_target_value(&event))
                />
            </div>
            <div class="grid gap-tight">
                <label for=version_id.clone() class=styles::LABEL>
                    "Code system version"
                </label>
                <input
                    id=version_id
                    name="systemVersion"
                    type="text"
                    class=styles::INPUT
                    aria-describedby=format!("compose-system-version-note-{key}")
                    prop:value=move || system_version.get()
                    on:input=move |event| {
                        let value = event_target_value(&event);
                        editing
                            .change(move |draft| {
                                if let Some(clause) = draft.clause_mut(key) {
                                    clause.system_version = value;
                                }
                            });
                    }
                />
                <p id=format!("compose-system-version-note-{key}") class=styles::HINT>
                    "compose.include.version. Left empty, the server resolves the version itself."
                </p>
            </div>
        </div>
    }
    .into_any()
}

/// The codes one clause names, and the search that picks them.
fn concept_rows(editing: Editing, offers: Offers, key: u32, clause: Memo<Clause>) -> AnyView {
    let rows: Memo<Vec<u32>> = Memo::new(move |_| {
        clause.with(|clause| clause.concepts.iter().map(|row| row.key).collect())
    });
    let listed = view! {
        <For
            each=move || rows.get()
            key=|row| *row
            children=move |row| concept_row(editing, key, row, clause)
        />
    }
    .into_any();
    let search = concept_search(editing, offers, key, clause);
    group(
        "Codes",
        vec![
            view! { <div class="grid gap-tight">{listed}</div> }.into_any(),
            search,
        ],
    )
}

/// One named code, with the display its author gives it.
fn concept_row(editing: Editing, key: u32, row: u32, clause: Memo<Clause>) -> AnyView {
    let picked = Memo::new(move |_| {
        clause.with(|clause| {
            clause
                .concepts
                .iter()
                .find(|held| held.key == row)
                .cloned()
                .unwrap_or_default()
        })
    });
    let included = Memo::new(move |_| clause.with(|clause| clause.included));
    let display = Memo::new(move |_| picked.with(|picked| picked.display.clone()));
    let control_id = format!("compose-display-{key}-{row}");
    view! {
        <div class="grid gap-tight">
            <p class=styles::CODE>
                {move || picked.with(|picked| picked.code.clone())} " "
                <span class=styles::CODE_MUTED>
                    {move || picked.with(|picked| picked.served_display.clone())}
                </span>
            </p>
            <div class="flex items-center gap-default">
                <Show when=move || included.get() fallback=|| ()>
                    <label for=control_id.clone() class="sr-only">
                        "The display this value set gives that code"
                    </label>
                    <input
                        id=control_id.clone()
                        name="display"
                        type="text"
                        placeholder="The display this value set gives it"
                        class=styles::INPUT
                        prop:value=move || display.get()
                        on:input=move |event| {
                            let typed = event_target_value(&event);
                            editing
                                .change(move |draft| {
                                    if let Some(row) = draft
                                        .clause_mut(key)
                                        .and_then(|clause| {
                                            clause.concepts.iter_mut().find(|held| held.key == row)
                                        })
                                    {
                                        row.display = typed;
                                    }
                                });
                        }
                    />
                </Show>
                <button
                    type="button"
                    class=styles::BUTTON_QUIET
                    on:click=move |_| {
                        editing
                            .change(move |draft| {
                                if let Some(clause) = draft.clause_mut(key) {
                                    clause.concepts.retain(|held| held.key != row);
                                }
                            });
                    }
                >
                    "Remove"
                </button>
            </div>
        </div>
    }
    .into_any()
}

/// The search that picks a code out of the clause's own code system.
///
/// A code is picked from what the server answers rather than typed, so a
/// clause cannot name a code the system does not hold. The search is the same
/// `$expand` over an inline value set the concept browser uses.
fn concept_search(editing: Editing, offers: Offers, key: u32, clause: Memo<Clause>) -> AnyView {
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let open = move || offers.picking.get() == Some(key);
    let system = Memo::new(move |_| clause.with(|held| held.system.clone()));
    let system_version = Memo::new(move |_| clause.with(|held| held.system_version.clone()));
    let query: Memo<Option<ConceptQuery>> = Memo::new(move |_| {
        if offers.picking.get() != Some(key) {
            return None;
        }
        let system = system.get();
        let term = offers.term.get();
        (!system.is_empty() && !term.is_empty()).then(|| ConceptQuery {
            system,
            system_version: Some(system_version.get()).filter(|held| !held.is_empty()),
            filter: Some(term),
            child_of: None,
            display_language: None,
            count: SEARCH_COUNT,
        })
    });
    let found = LocalResource::new(move || {
        let client = client.clone();
        let version = version.get();
        let query = query.get();
        async move {
            match query {
                Some(query) => Some(client.expand_inline(version, &query).await),
                None => None,
            }
        }
    });
    let typed: NodeRef<Input> = NodeRef::new();
    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        let asked = typed.get().map(|input| input.value()).unwrap_or_default();
        offers.picking.set(Some(key));
        offers.term.set(asked.trim().to_owned());
    };
    let results = move || {
        found.with(|answered| {
            answered
                .as_ref()
                .and_then(Option::as_ref)
                .map(|result| match result {
                    Ok(value) => found_view(editing, key, value),
                    Err(error) => view! { <Failure error=error.clone() /> }.into_any(),
                })
        })
    };
    view! {
        <div class="grid gap-tight">
            <form class="flex items-end gap-default" on:submit=submit>
                <div class="grid min-w-0 flex-1 gap-tight">
                    <label for=format!("compose-search-{key}") class=styles::LABEL>
                        "Find a code in that code system"
                    </label>
                    <input
                        id=format!("compose-search-{key}")
                        name="q"
                        type="search"
                        class=styles::INPUT
                        node_ref=typed
                        disabled=move || system.with(String::is_empty)
                    />
                </div>
                <button
                    type="submit"
                    class=styles::BUTTON
                    disabled=move || system.with(String::is_empty)
                >
                    <Icon glyph=icon::SEARCH />
                    "Search"
                </button>
            </form>
            <Show when=open fallback=|| ()>
                <Reading label="Searching that code system">{results}</Reading>
            </Show>
        </div>
    }
    .into_any()
}

/// The concepts a search answered, each with the control that adds it.
fn found_view(editing: Editing, key: u32, value: &ExpandedValueSet) -> AnyView {
    let Some(expansion) = value.expansion() else {
        return crate::components::state::empty(
            "The server answered no expansion for that search.",
        );
    };
    if expansion.concepts.is_empty() {
        return crate::components::state::empty(
            "No concept of that code system matches that text.",
        );
    }
    let rows: Vec<AnyView> = expansion
        .concepts
        .iter()
        .map(|concept| found_row(editing, key, concept))
        .collect();
    view! { <ul class="grid gap-tight">{rows}</ul> }.into_any()
}

/// One search result, and the control that adds it to the clause.
fn found_row(editing: Editing, key: u32, concept: &ConceptRow) -> AnyView {
    let code = concept.code.clone();
    let display = concept.display.clone().unwrap_or_default();
    let shown_code = code.clone();
    let shown_display = display.clone();
    view! {
        <li class="flex items-center justify-between gap-default">
            <span class=styles::CODE>
                {shown_code} " " <span class=styles::CODE_MUTED>{shown_display}</span>
            </span>
            <button
                type="button"
                class=styles::BUTTON
                on:click=move |_| {
                    let code = code.clone();
                    let display = display.clone();
                    editing.change(move |draft| draft.add_concept(key, &code, &display));
                }
            >
                "Add"
            </button>
        </li>
    }
    .into_any()
}

/// The filters one clause selects with, drawn from what the version declares.
fn filter_rows(
    editing: Editing,
    offers: Offers,
    key: u32,
    clause: Memo<Clause>,
    system: Memo<String>,
) -> AnyView {
    let rows: Memo<Vec<u32>> = Memo::new(move |_| {
        clause.with(|clause| clause.filters.iter().map(|row| row.key).collect())
    });
    let declared = Memo::new(move |_| offers.filters(&system.get()));
    let listed = view! {
        <For
            each=move || rows.get()
            key=|row| *row
            children=move |row| filter_row(editing, key, row, clause)
        />
    }
    .into_any();
    let chosen = RwSignal::new(String::new());
    let options = move || {
        declared
            .get()
            .into_iter()
            .flat_map(|filter| {
                filter.operators.into_iter().map({
                    let code = filter.code.clone();
                    move |operator| {
                        let value = format!("{code} {operator}");
                        let shown = value.clone();
                        view! { <option value=value>{shown}</option> }.into_any()
                    }
                })
            })
            .collect::<Vec<AnyView>>()
    };
    let adder = view! {
        <div class="grid gap-tight">
            <label for=format!("compose-add-filter-{key}") class=styles::LABEL>
                "Add a filter this server declares"
            </label>
            <div class="flex items-center gap-default">
                <select
                    id=format!("compose-add-filter-{key}")
                    class=styles::INPUT
                    prop:value=move || chosen.get()
                    on:change=move |event| chosen.set(event_target_value(&event))
                >
                    <option value="">"Choose a property and an operator"</option>
                    {options}
                </select>
                <button
                    type="button"
                    class=styles::BUTTON
                    on:click=move |_| {
                        let picked = chosen.get_untracked();
                        let Some((property, operator)) = picked.split_once(' ') else {
                            return;
                        };
                        let property = property.to_owned();
                        let operator = operator.to_owned();
                        editing.change(move |draft| draft.add_filter(key, &property, &operator));
                        chosen.set(String::new());
                    }
                >
                    "Add"
                </button>
            </div>
            <Show
                when=move || !system.with(String::is_empty) && declared.with(Vec::is_empty)
                fallback=|| ()
            >
                <p class=styles::HINT>
                    "This server declares no filter for that code system version."
                </p>
            </Show>
        </div>
    }
    .into_any();
    group(
        "Filters",
        vec![
            view! { <div class="grid gap-tight">{listed}</div> }.into_any(),
            adder,
        ],
    )
}

/// One filter, with the value it applies.
fn filter_row(editing: Editing, key: u32, row: u32, clause: Memo<Clause>) -> AnyView {
    let held = Memo::new(move |_| {
        clause.with(|clause| {
            clause
                .filters
                .iter()
                .find(|filter| filter.key == row)
                .cloned()
                .unwrap_or_default()
        })
    });
    let value = Memo::new(move |_| held.with(|held| held.value.clone()));
    let control_id = format!("compose-filter-{key}-{row}");
    view! {
        <div class="grid gap-tight">
            <label for=control_id.clone() class=styles::LABEL>
                {move || held.with(|held| format!("{} {}", held.property, held.op))}
            </label>
            <div class="flex items-center gap-default">
                <input
                    id=control_id
                    name="filterValue"
                    type="text"
                    class=styles::INPUT
                    prop:value=move || value.get()
                    on:input=move |event| {
                        let typed = event_target_value(&event);
                        editing
                            .change(move |draft| {
                                if let Some(filter) = draft
                                    .clause_mut(key)
                                    .and_then(|clause| {
                                        clause.filters.iter_mut().find(|held| held.key == row)
                                    })
                                {
                                    filter.value = typed;
                                }
                            });
                    }
                />
                <button
                    type="button"
                    class=styles::BUTTON_QUIET
                    on:click=move |_| {
                        editing
                            .change(move |draft| {
                                if let Some(clause) = draft.clause_mut(key) {
                                    clause.filters.retain(|held| held.key != row);
                                }
                            });
                    }
                >
                    "Remove"
                </button>
            </div>
        </div>
    }
    .into_any()
}

/// The save control, and what a refusal does to the screen.
#[expect(
    clippy::too_many_arguments,
    reason = "the save carries the client, the address, the draft, the rights, and the three signals a refusal writes"
)]
fn save_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    editing: Editing,
    writable: Memo<bool>,
    session: Session,
    report: RwSignal<String>,
    refusal: RwSignal<Option<FhirError>>,
    saves: RwSignal<u32>,
    previewed: RwSignal<Option<Draft>>,
) -> AnyView {
    let client = client.clone();
    let navigate = StoredValue::new(use_navigate());
    let saving = RwSignal::new(false);
    let savable = editing.part(Draft::savable);
    let save = move |_| {
        if saving.get_untracked() {
            return;
        }
        saving.set(true);
        refusal.set(None);
        report.set(String::from("Saving"));
        let client = client.clone();
        let version = version.get_untracked();
        let draft = editing.draft();
        let token = session.token();
        let body = draft.resource().to_string();
        spawn_local(async move {
            let written = if draft.saved() {
                Box::pin(
                    client.update(
                        version,
                        VALUE_SET,
                        &draft.id,
                        &body,
                        Some(&draft.version_id)
                            .filter(|held| !held.is_empty())
                            .map(String::as_str),
                        token.as_deref(),
                    ),
                )
                .await
            } else {
                Box::pin(client.create(version, VALUE_SET, &body, token.as_deref())).await
            };
            saving.set(false);
            match written {
                Ok(answer) => {
                    let id = answer
                        .resource
                        .as_ref()
                        .and_then(|resource| resource.id.clone())
                        .unwrap_or_else(|| draft.id.clone());
                    report.set(String::from("Saved."));
                    editing.reopen();
                    previewed.set(None);
                    saves.update(|counted| *counted = counted.saturating_add(1));
                    if id != draft.id {
                        let target = base_url()
                            .segment(COMPOSE_PATH)
                            .query(VERSION_PARAM, version.segment())
                            .query(ID_PARAM, &id)
                            .render("");
                        navigate.with_value(|navigate| {
                            navigate(
                                &target,
                                NavigateOptions {
                                    resolve: false,
                                    ..NavigateOptions::default()
                                },
                            );
                        });
                    }
                }
                Err(error) => {
                    let refused = Refusal::of(&error);
                    if refused.drops_the_token() {
                        session.release();
                    }
                    report.set(refused.what_to_do().to_owned());
                    refusal.set(Some(error));
                }
            }
        });
    };
    view! {
        <div class="mt-section flex flex-wrap items-center gap-default">
            <button
                type="button"
                class=styles::SUBMIT
                disabled=move || !writable.get() || !savable.get() || saving.get()
                on:click=save
            >
                {move || if saving.get() { "Saving" } else { "Save this value set" }}
            </button>
            {help_toggle()}
            <Show when=move || writable.get() && !savable.get() fallback=|| ()>
                <p role="status" class=styles::HINT>
                    "A canonical and a clause that selects something are what a save needs."
                </p>
            </Show>
        </div>
    }
    .into_any()
}

/// What the server said when it refused a write, in its own words.
fn refusal_section(refusal: RwSignal<Option<FhirError>>, editing: Editing) -> AnyView {
    let concurrent = Memo::new(move |_| {
        refusal.with(|held| {
            held.as_ref()
                .is_some_and(|error| Refusal::of(error) == Refusal::ConcurrentEdit)
        })
    });
    let shown = move || {
        refusal.with(|held| {
            held.clone()
                .map(|error| view! { <Failure error=error /> }.into_any())
        })
    };
    view! {
        <div class="mt-default grid gap-default">
            {shown} <Show when=move || concurrent.get() fallback=|| ()>
                <button
                    type="button"
                    class=styles::BUTTON
                    on:click=move |_| {
                        editing.reopen();
                        refusal.set(None);
                    }
                >
                    "Reload the value set as the server holds it"
                </button>
            </Show>
        </div>
    }
    .into_any()
}

/// The preview read: the answer to `$expand` over the compose on screen.
type Previewing = LocalResource<Option<Result<ExpandedValueSet, FhirError>>>;

/// Reads `$expand` over the draft the reader asked to preview.
fn previewing(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    previewed: RwSignal<Option<Draft>>,
    page: Memo<Preview>,
) -> Previewing {
    let client = client.clone();
    LocalResource::new(move || {
        let client = client.clone();
        let version = version.get();
        let draft = previewed.get();
        let page = page.get();
        async move {
            match draft {
                Some(draft) => Some(client.preview_compose(version, &draft, &page).await),
                None => None,
            }
        }
    })
}

/// What a preview answered, as the sentence the live region reads out.
fn announcing(expansion: Previewing) -> Memo<String> {
    Memo::new(move |_| {
        expansion.with(|answered| {
            answered
                .as_ref()
                .and_then(Option::as_ref)
                .map(|result| match result {
                    Ok(value) => count_sentence(value),
                    Err(error) => summary_of(error),
                })
                .unwrap_or_default()
        })
    })
}

/// The preview: `$expand` over the compose as it stands, one page at a time.
fn preview_section(
    expansion: Previewing,
    previewed: RwSignal<Option<Draft>>,
    page: Memo<Preview>,
    editing: Editing,
    report: RwSignal<String>,
) -> AnyView {
    let shown = move || {
        expansion.with(|answered| {
            answered.as_ref().map(|answered| match answered {
                None => crate::components::state::invitation(
                    "Run the preview to see what this definition selects. It expands the definition on screen, saved or not.",
                ),
                Some(Ok(value)) => preview_view(value, page, editing),
                Some(Err(error)) => expansion_refusal(error, editing),
            })
        })
    };
    let controls = preview_controls(previewed, editing, page, report);
    view! {
        <section class="mt-loose" aria-labelledby="compose-preview-heading">
            <h2 id="compose-preview-heading" class=styles::SECTION_TITLE>
                "The preview"
            </h2>
            {controls}
            <Reading label="Expanding the definition">{shown}</Reading>
        </section>
    }
    .into_any()
}

/// The preview's own controls: the text filter and the run.
fn preview_controls(
    previewed: RwSignal<Option<Draft>>,
    editing: Editing,
    page: Memo<Preview>,
    report: RwSignal<String>,
) -> AnyView {
    let navigate = StoredValue::new(use_navigate());
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let id = editing.part(|draft| draft.id.clone());
    let filter: NodeRef<Input> = NodeRef::new();
    let seeded = Memo::new(move |_| page.with(|page| page.filter.clone().unwrap_or_default()));
    let run = move |event: SubmitEvent| {
        event.prevent_default();
        report.set(String::new());
        previewed.set(Some(editing.draft()));
        let typed = filter.get().map(|input| input.value()).unwrap_or_default();
        let target = address(
            &id.get_untracked(),
            version.get_untracked(),
            &Preview {
                filter: Some(typed.trim().to_owned()).filter(|typed| !typed.is_empty()),
                count: page.get_untracked().count,
                offset: 0,
            },
        );
        navigate.with_value(|navigate| {
            navigate(
                &target,
                NavigateOptions {
                    resolve: false,
                    ..NavigateOptions::default()
                },
            );
        });
    };
    view! {
        <form class="mt-default flex items-end gap-default" on:submit=run>
            <div class="grid min-w-0 flex-1 gap-tight">
                <label for="compose-preview-filter" class=styles::LABEL>
                    "Filter the preview"
                </label>
                <input
                    id="compose-preview-filter"
                    name="filter"
                    type="search"
                    class=styles::INPUT
                    node_ref=filter
                    prop:value=move || seeded.get()
                />
            </div>
            <button type="submit" class=styles::SUBMIT>
                <Icon glyph=icon::EXPAND />
                "Run the preview"
            </button>
        </form>
    }
    .into_any()
}

/// The screen's own address, with one preview page selected.
fn address(id: &str, version: FhirVersion, page: &Preview) -> String {
    let mut url = base_url()
        .segment(COMPOSE_PATH)
        .query(VERSION_PARAM, version.segment());
    if !id.is_empty() {
        url = url.query(ID_PARAM, id);
    }
    if let Some(filter) = page.filter.as_ref() {
        url = url.query(FILTER_PARAM, filter);
    }
    url = url.query(COUNT_PARAM, &page.count.to_string());
    if page.offset > 0 {
        url = url.query(OFFSET_PARAM, &page.offset.to_string());
    }
    url.render("")
}

/// The page the preview answered, with the walk through the rest of it.
fn preview_view(value: &ExpandedValueSet, page: Memo<Preview>, editing: Editing) -> AnyView {
    let Some(expansion) = value.expansion() else {
        return crate::components::state::empty(
            "The server answered a ValueSet carrying no expansion.",
        );
    };
    let rows: Vec<AnyView> = expansion
        .concepts
        .iter()
        .map(|concept| {
            view! {
                <tr>
                    <td class=styles::TD_TIGHT>
                        <span class=styles::CODE>{concept.code.clone()}</span>
                    </td>
                    <td class=styles::TD>{concept.display.clone().unwrap_or_default()}</td>
                </tr>
            }
            .into_any()
        })
        .collect();
    let total = expansion.total;
    let listed = expansion.listed();
    let walk = pager(page, total, listed, editing);
    view! {
        <table class=format!("mt-default {}", styles::TABLE)>
            <thead>
                <tr>
                    <th scope="col" class=styles::TH>
                        "Code"
                    </th>
                    <th scope="col" class=styles::TH>
                        "Display"
                    </th>
                </tr>
            </thead>
            <tbody>{rows}</tbody>
        </table>
        {walk}
    }
    .into_any()
}

/// The links that walk the preview's pages.
fn pager(page: Memo<Preview>, total: Option<u32>, rows: u32, editing: Editing) -> AnyView {
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let current = page.get();
    let id = editing.draft().id;
    let at = Page::at(current.offset, current.count);
    let previous = at.previous();
    let next = match total {
        Some(total) => at.next(total),
        None => (rows >= at.count())
            .then(|| Page::at(at.offset().saturating_add(at.count()), at.count())),
    };
    let link = move |target: Page, label: &'static str| {
        let address = address(
            &id,
            version.get(),
            &Preview {
                filter: current.filter.clone(),
                count: target.count(),
                offset: target.offset(),
            },
        );
        view! {
            <a href=address class=styles::BUTTON>
                {label}
            </a>
        }
        .into_any()
    };
    let counted = match total {
        Some(total) => format!("{rows} of {total} concepts on this page."),
        None => format!("{rows} concepts on this page. The server declared no total."),
    };
    view! {
        <div class="mt-default flex items-center gap-default">
            {previous.map(|target| link(target, "Previous"))}
            {next.map(|target| link(target, "Next"))} <p class=styles::MUTED>{counted}</p>
        </div>
    }
    .into_any()
}

/// A refused preview: the server's outcome, and the character it points at.
fn expansion_refusal(error: &FhirError, editing: Editing) -> AnyView {
    let outcome = error
        .outcome()
        .map(OperationOutcome::lines)
        .unwrap_or_default();
    let marked: Vec<AnyView> = outcome
        .iter()
        .filter_map(|line| marked_expression(&line.text, editing))
        .collect();
    let shown = error.clone();
    view! {
        <div class="mt-default grid gap-default">
            <Failure error=Signal::stored(shown) />
            {marked}
        </div>
    }
    .into_any()
}

/// The expression the refusal is about, with the character it points at marked.
///
/// The position is the server's own (`crate::fhir::implicit::position_in`), and
/// the expression is the one whose canonical the outcome names, so nothing here
/// parses anything.
fn marked_expression(diagnostic: &str, editing: Editing) -> Option<AnyView> {
    let position = implicit::position_in(diagnostic)?;
    let draft = editing.draft();
    let (expression, form) = draft.clauses.iter().find_map(|clause| {
        clause.value_sets.iter().find_map(|row| {
            diagnostic
                .contains(&row.canonical)
                .then(|| implicit::read(&row.canonical))
                .flatten()
                .filter(|(_, form, _)| form.expression)
                .map(|(_, form, value)| (value, form))
        })
    })?;
    let (before, at, after) = implicit::mark(&expression, position);
    Some(
        view! {
            <div class=format!("rounded-md panel-p {}", styles::NOTICE)>
                <p class=styles::LABEL>
                    {format!("{}, at character {}", form.label, position.saturating_add(1))}
                </p>
                <p class=format!(
                    "mt-tight {}",
                    styles::CODE,
                )>{before} <span class="font-semibold underline">{at}</span> {after}</p>
            </div>
        }
        .into_any(),
    )
}

/// What a preview answered, as the sentence the live region reads out.
fn count_sentence(value: &ExpandedValueSet) -> String {
    let Some(expansion) = value.expansion() else {
        return String::from("The server answered no expansion for this definition.");
    };
    match expansion.total {
        Some(total) => format!("This definition selects {total} concepts."),
        None => format!(
            "{} concepts on this page. The server declared no total.",
            expansion.listed()
        ),
    }
}

/// What a refused preview says, as one sentence the live region reads out.
fn summary_of(error: &FhirError) -> String {
    let said = error
        .outcome()
        .map(OperationOutcome::lines)
        .unwrap_or_default()
        .into_iter()
        .map(|line| line.text)
        .collect::<Vec<String>>()
        .join(" ");
    if said.is_empty() {
        String::from("The server refused the preview.")
    } else {
        format!("The server refused the preview: {said}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preview_address_carries_the_page_it_shows() {
        assert_eq!(
            address(
                "local",
                FhirVersion::R4B,
                &Preview {
                    filter: Some(String::from("a b")),
                    count: 20,
                    offset: 40,
                }
            ),
            format!(
                "{}/{COMPOSE_PATH}?fhir=r4b&id=local&filter=a%20b&count=20&offset=40",
                crate::routes::UI_BASE
            ),
            "every value is percent-encoded into the query it belongs to"
        );
    }

    #[test]
    fn an_unsaved_draft_addresses_no_resource() {
        let address = address(
            "",
            FhirVersion::R5,
            &Preview {
                filter: None,
                count: 20,
                offset: 0,
            },
        );
        assert!(!address.contains("id="), "{address}");
        assert!(!address.contains("offset="), "{address}");
        assert!(address.ends_with("count=20"), "{address}");
    }
}

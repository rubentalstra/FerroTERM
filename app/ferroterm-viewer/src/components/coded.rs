//! A control over a code of a value set the served root expands.
//!
//! Every coded element of an authored resource is bound to a value set
//! (<https://hl7.org/fhir/R4B/terminologies.html#binding>), and the editor
//! offers what the root expands that value set to rather than a list compiled
//! into the bundle. One control draws all of them, so a root that admits
//! another code offers it on every screen without a new build.

use leptos::ev::Event;
use leptos::prelude::*;

use crate::fhir::FhirClient;
use crate::fhir::version::FhirVersion;
use crate::styles;

/// One code offered by a coded control.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Coded {
    /// The code itself, which is what the resource carries.
    pub(crate) code: String,
    /// The display the server sent for it, empty where it sent none.
    pub(crate) display: String,
}

/// The codes one coded control offers, and why it has none when it has none.
#[derive(Clone, Copy)]
pub(crate) struct Codes {
    /// The codes the value set expanded to.
    pub(crate) offered: Signal<Vec<Coded>>,
    /// What the server said instead, when the expansion did not answer.
    pub(crate) refused: Signal<Option<String>>,
}

/// What one coded control is called, and how its label is drawn.
///
/// The four travel together so the control takes one argument for its naming
/// rather than four the caller could pass in the wrong order.
pub(crate) struct Control {
    /// The `id` the label points at.
    pub(crate) id: String,
    /// The `name` the control carries.
    pub(crate) name: &'static str,
    /// The label a reader reads.
    pub(crate) label: &'static str,
    /// Whether the label is for a screen reader alone, because a column
    /// header already names the control for a sighted reader.
    pub(crate) sr_only: bool,
}

/// The codes one value set expands to, as the signals a control reads.
///
/// `canonical` is a signal because the value set an element is bound to can
/// differ per served version, and the version switcher changes it without
/// leaving the route.
///
/// A value set the root refuses to expand leaves the control with the code the
/// resource already carries and nothing else, and the control says so in the
/// server's own words rather than looking like a list with one entry in it.
pub(crate) fn codes_of(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    canonical: Signal<String>,
) -> Codes {
    let client = client.clone();
    let expanded = LocalResource::new(move || {
        let client = client.clone();
        let version = version.get();
        let canonical = canonical.get();
        async move { client.value_set_codes(version, &canonical).await }
    });
    Codes {
        offered: Signal::derive(move || {
            expanded.with(|answered| {
                answered
                    .as_ref()
                    .and_then(|read| read.as_ref().ok())
                    .map(|rows| {
                        rows.iter()
                            .map(|row| Coded {
                                code: row.code.clone(),
                                display: row.display.clone().unwrap_or_default(),
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            })
        }),
        refused: Signal::derive(move || {
            expanded.with(|answered| {
                answered
                    .as_ref()
                    .and_then(|read| read.as_ref().err().map(ToString::to_string))
            })
        }),
    }
}

/// One control over a code of a value set the server expanded.
///
/// The select is driven by `prop:value` because its options arrive after the
/// form is built, and the code the resource already carries is offered whether
/// or not the expansion did: a control that dropped it would silently rewrite
/// the resource on the next save.
pub(crate) fn coded_control(
    control: Control,
    codes: Codes,
    readonly: Signal<bool>,
    held: Signal<String>,
    mut chose: impl FnMut(String) + 'static,
) -> AnyView {
    let Control {
        id,
        name,
        label,
        sr_only,
    } = control;
    let offered = move || {
        let held = held.get();
        let mut drawn: Vec<AnyView> = Vec::new();
        let listed = codes
            .offered
            .with(|codes| codes.iter().any(|coded| coded.code == held));
        if !listed {
            let text = if held.is_empty() {
                "Choose one".to_owned()
            } else {
                held.clone()
            };
            drawn.push(view! { <option value=held.clone()>{text}</option> }.into_any());
        }
        drawn.extend(codes.offered.with(|codes| {
            codes
                .iter()
                .map(|coded| {
                    let code = coded.code.clone();
                    let text = if coded.display.is_empty() {
                        coded.code.clone()
                    } else {
                        format!("{} ({})", coded.display, coded.code)
                    };
                    view! { <option value=code>{text}</option> }.into_any()
                })
                .collect::<Vec<AnyView>>()
        }));
        drawn
    };
    let named = StoredValue::new(id);
    let label_class = if sr_only { "sr-only" } else { styles::LABEL };
    view! {
        <div class="grid gap-tight">
            <label for=move || named.with_value(Clone::clone) class=label_class>
                {label}
            </label>
            <select
                id=move || named.with_value(Clone::clone)
                name=name
                class=styles::INPUT
                disabled=move || readonly.get()
                prop:value=move || held.get()
                on:change=move |event: Event| chose(event_target_value(&event))
            >
                {offered}
            </select>
            <Show when=move || codes.refused.with(Option::is_some) fallback=|| ()>
                <p role="status" class=styles::HINT>
                    "This server did not expand the value set this control's codes come from, so it offers only what the resource already carries: "
                    {move || codes.refused.get()}
                </p>
            </Show>
        </div>
    }
    .into_any()
}

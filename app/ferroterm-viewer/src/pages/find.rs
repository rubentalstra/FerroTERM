//! What the command bar was given, and what this root can do with it.
//!
//! The classification is the viewer's own reading of the string's shape, in
//! `crate::find`, and it happens before any request. What gates the offers is
//! the one request this screen makes: the root's `CapabilityStatement`, which
//! is where a server states the operations it answers per resource type
//! (<https://hl7.org/fhir/R5/capabilitystatement.html>). An operation it does
//! not declare is left out rather than offered and refused.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_query_map;

use crate::components::failure::Failure;
use crate::components::reading::Reading;
use crate::components::shell::SelectedVersion;
use crate::fhir::CODE_SYSTEM;
use crate::fhir::CONCEPT_MAP;
use crate::fhir::FhirClient;
use crate::fhir::VALUE_SET;
use crate::fhir::capability::CapabilityStatement;
use crate::find::Declared;
use crate::find::Offer;
use crate::find::QUERY_PARAM;
use crate::find::Subject;
use crate::find::offers;
use crate::styles;

/// The `$expand` operation, as a capability statement names it.
const EXPAND: &str = "expand";

/// The `$validate-code` operation, as a capability statement names it.
const VALIDATE_CODE: &str = "validate-code";

/// The `$translate` operation, as a capability statement names it.
const TRANSLATE: &str = "translate";

/// Shows what the command bar was given and where it can go.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn FindPage() -> impl IntoView {
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let query = use_query_map();
    let typed = Memo::new(move |_| {
        query.with(|map| map.get(QUERY_PARAM).unwrap_or_default().trim().to_owned())
    });
    let subject = Memo::new(move |_| typed.with(|typed| Subject::of(typed)));

    let read_client = client.clone();
    let statement = LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        async move { client.capability_statement(version).await }
    });

    let heading = view! {
        <Title text="Find" />
        <h1 class=styles::PAGE_TITLE>"What to do with it"</h1>
        <p class=styles::LEAD>
            {move || {
                subject
                    .with(|subject| match subject {
                        Some(subject) => {
                            format!("Read as {} from its shape alone.", subject.label())
                        }
                        None => "Nothing was typed.".to_owned(),
                    })
            }}
        </p>
    }
    .into_any();

    let subject_view = move || {
        subject.with(|subject| {
            subject.as_ref().map(|subject| {
                view! { <p class=format!("mt-default {}", styles::CODE)>{subject.typed().to_owned()}</p> }
                    .into_any()
            })
        })
    };

    let body = move || {
        statement.with(|answered| {
            answered.as_ref().map(|result| match result {
                Ok(statement) => subject.with(|subject| match subject {
                    Some(subject) => {
                        offers_view(&offers(subject, declared_by(statement), version.get()))
                    }
                    None => ().into_any(),
                }),
                Err(error) => {
                    let error = error.clone();
                    view! {
                        <div class="mt-default">
                            <Failure error=Signal::stored(error) />
                        </div>
                    }
                    .into_any()
                }
            })
        })
    };

    view! {
        {heading}
        {subject_view}
        <section class="mt-loose" aria-labelledby="find-offers-heading">
            <h2 id="find-offers-heading" class=styles::SECTION_TITLE>
                "Where it goes"
            </h2>
            <Reading label="Reading what this root declares">{body}</Reading>
        </section>
    }
}

/// What the root declares, as the offers read it.
fn declared_by(statement: &CapabilityStatement) -> Declared {
    Declared {
        validate: statement.declares_operation(CODE_SYSTEM, VALIDATE_CODE)
            || statement.declares_operation(VALUE_SET, VALIDATE_CODE),
        expand: statement.declares_operation(VALUE_SET, EXPAND),
        translate: statement.declares_operation(CONCEPT_MAP, TRANSLATE),
    }
}

/// The offers, as the list of links they are.
fn offers_view(found: &[Offer]) -> AnyView {
    if found.is_empty() {
        return view! {
            <p class=format!(
                "mt-default {}",
                styles::MUTED,
            )>"This root declares none of the operations that would apply to what you typed."</p>
        }
        .into_any();
    }
    let rows: Vec<AnyView> = found
        .iter()
        .map(|offer| {
            let href = offer.href.clone();
            view! {
                <li>
                    <a href=href class=format!("block panel-p {}", styles::PANEL)>
                        <span class=format!("block {}", styles::LINK)>{offer.label}</span>
                        <span class=format!(
                            "mt-tight block {}",
                            styles::MUTED,
                        )>{offer.detail}</span>
                    </a>
                </li>
            }
            .into_any()
        })
        .collect();
    view! { <ul class="mt-default grid gap-default">{rows}</ul> }.into_any()
}

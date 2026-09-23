//! Where the identity provider sends the reader back.
//!
//! The screen reads `code` and `state` off the address, checks the state
//! against the one this browser sent (RFC 6749 §10.12), exchanges the code for
//! a token, and returns the reader to the overview. A refusal renders here, in
//! the issuer's own words.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::NavigateOptions;
use leptos_router::hooks::use_navigate;
use leptos_router::hooks::use_query;
use leptos_router::params::Params;
use leptos_router::params::ParamsError;

use crate::auth::Returned;
use crate::auth::Session;
use crate::auth::SignInError;
use crate::auth::complete;
use crate::components::shell::SelectedVersion;
use crate::fhir::FhirClient;
use crate::fhir::smart::SmartConfiguration;
use crate::routes::OVERVIEW_PATH;
use crate::routes::ui_link;
use crate::styles;

/// What the issuer put in the redirect (RFC 6749 §4.1.2 and §4.1.2.1).
#[derive(Clone, Debug, Params, PartialEq)]
struct CallbackQuery {
    /// The authorization code, on success.
    code: Option<String>,
    /// The `state` this browser sent with the request.
    state: Option<String>,
    /// The `error` code, on a refusal.
    error: Option<String>,
    /// The issuer's own sentence about the refusal.
    error_description: Option<String>,
}

/// How the sign-in ended.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Ending {
    /// The token is held, and the reader is on their way back.
    Done,
    /// The sign-in did not complete, with what to tell the reader.
    Failed {
        /// The viewer's own sentence.
        message: String,
        /// The issuer's `error_description`, when it sent one.
        detail: Option<String>,
    },
}

/// Finishes the sign-in the shell started.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn CallbackPage() -> impl IntoView {
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let session = expect_context::<Session>();
    let navigate = StoredValue::new(use_navigate());
    let query = use_query::<CallbackQuery>();

    let exchange = LocalResource::new({
        let client = client.clone();
        move || {
            let client = client.clone();
            let returned = query.with(read_returned);
            let version = version.get();
            async move {
                let document = client.smart_configuration(version).await;
                let Some(sign_in) = document.as_ref().ok().and_then(SmartConfiguration::sign_in)
                else {
                    return Ending::Failed {
                        message: String::from(
                            "This server publishes no sign-in, so there is nothing to complete.",
                        ),
                        detail: None,
                    };
                };
                match complete(&sign_in, &client.redirect_uri(), &returned).await {
                    Ok(access) => {
                        session.hold(access);
                        Ending::Done
                    }
                    Err(error) => Ending::Failed {
                        message: error.to_string(),
                        detail: detail_of(&error),
                    },
                }
            }
        }
    });

    // Sending the reader on is a navigation, which is the outside world and
    // what an Effect is for. It writes no signal: the screen below reads the
    // same resource for itself.
    Effect::new(move |_| {
        let done = exchange.with(|ending| ending.as_ref() == Some(&Ending::Done));
        if !done {
            return;
        }
        let target = ui_link(OVERVIEW_PATH, version.get());
        navigate.with_value(|navigate| {
            navigate(
                &target,
                NavigateOptions {
                    resolve: false,
                    // The callback address carries a spent code, so the back
                    // button must not land the reader on it again.
                    replace: true,
                    ..NavigateOptions::default()
                },
            );
        });
    });

    let report = move || {
        exchange.with(|ending| match ending {
            None => view! { <p class=styles::MUTED>"Completing the sign-in"</p> }.into_any(),
            Some(Ending::Done) => view! { <p class=styles::MUTED>"Signed in"</p> }.into_any(),
            Some(Ending::Failed { message, detail }) => refused(message, detail.as_deref()),
        })
    };

    view! {
        <Title text="Signing in" />
        <section class="mx-auto max-w-2xl">
            <h1 class=styles::PAGE_TITLE>"Signing in"</h1>
            <div id="sign-in-outcome" aria-live="polite" class="mt-default">
                {report}
            </div>
            <p class="mt-default">
                <a class=styles::LINK href=move || ui_link(OVERVIEW_PATH, version.get())>
                    "Back to the overview"
                </a>
            </p>
        </section>
    }
}

/// What the redirect carried, read off the typed query.
///
/// A query that does not parse is a redirect the viewer cannot read, so it
/// carries nothing and fails the state check that follows.
fn read_returned(query: &Result<CallbackQuery, ParamsError>) -> Returned {
    query.as_ref().map_or_else(
        |_unreadable| Returned::default(),
        |query| Returned {
            code: query.code.clone(),
            state: query.state.clone(),
            error: query.error.clone(),
            error_description: query.error_description.clone(),
        },
    )
}

/// The issuer's own sentence about a refusal, when it sent one.
fn detail_of(error: &SignInError) -> Option<String> {
    match error {
        SignInError::Refused { description, .. } => description.clone(),
        SignInError::Pkce(_)
        | SignInError::NoCode
        | SignInError::StateMismatch
        | SignInError::NoVerifier
        | SignInError::Exchange(_) => None,
    }
}

/// A sign-in that did not complete, with whatever the issuer said about it.
fn refused(message: &str, detail: Option<&str>) -> AnyView {
    let said = detail.map(str::to_owned);
    let shown = said.clone();
    let sentence = message.to_owned();
    view! {
        <div
            role="alert"
            class="rounded-md border border-danger-soft-fg/30 bg-danger-soft p-default text-danger-soft-fg"
        >
            <p class="font-medium">{sentence}</p>
            <Show when=move || said.is_some() fallback=|| ()>
                <p class="mt-tight text-small">{shown.clone()}</p>
            </Show>
        </div>
    }
    .into_any()
}

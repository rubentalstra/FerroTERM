//! Where the identity provider sends the reader back.
//!
//! The screen reads `code` and `state` off the address, checks the state
//! against the one this browser sent (RFC 6749 §10.12), exchanges the code for
//! a token, and returns the reader to the overview. A refusal renders here, in
//! the words that came back.

use leptos::prelude::*;
use leptos::task::spawn_local;
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
use crate::components::failure::Failure;
use crate::components::shell::SelectedVersion;
use crate::fhir::FhirClient;
use crate::fhir::error::FhirError;
use crate::fhir::version::FhirVersion;
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
#[derive(Clone, Debug)]
enum Ending {
    /// The token is held, and the reader is on their way back.
    Done,
    /// The sign-in did not complete, with what to tell the reader.
    Failed {
        /// The viewer's own sentence, which the live region announces.
        message: String,
        /// Whatever came back beyond it, rendered below.
        detail: Detail,
    },
}

/// What a screen can show beyond the one sentence it announces.
#[derive(Clone, Debug)]
enum Detail {
    /// Nothing came back beyond the sentence itself.
    None,
    /// The issuer's own `error_description` (RFC 6749 §4.1.2.1).
    Said(String),
    /// A request that failed, with the status, the address, and the body.
    Read(Box<FhirError>),
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

    let ending: RwSignal<Option<Ending>> = RwSignal::new(None);
    // NOTE: an authorization code is spent once
    // (<https://www.rfc-editor.org/rfc/rfc6749#section-4.1.2>), so the exchange
    // runs once here rather than as a resource a navigation could re-run.
    let returned = query.with_untracked(read_returned);
    let selected = version.get_untracked();
    let target = ui_link(OVERVIEW_PATH, selected);
    spawn_local(async move {
        let reached = finish(&client, selected, &returned, session).await;
        let done = matches!(reached, Ending::Done);
        ending.set(Some(reached));
        if done {
            navigate.with_value(|navigate| {
                navigate(
                    &target,
                    NavigateOptions {
                        resolve: false,
                        // The callback address carries a spent code, so the
                        // back button must not land the reader on it again.
                        replace: true,
                        ..NavigateOptions::default()
                    },
                );
            });
        }
    });

    let announced = move || {
        ending.with(|reached| match reached {
            None => String::from("Completing the sign-in"),
            Some(Ending::Done) => String::from("Signed in"),
            Some(Ending::Failed { message, .. }) => message.clone(),
        })
    };
    let detail = move || {
        ending.with(|reached| match reached {
            None | Some(Ending::Done) => ().into_any(),
            Some(Ending::Failed { detail, .. }) => drawn(detail),
        })
    };

    let outcome = view! {
        <p id="sign-in-outcome" aria-live="polite" class=format!("mt-default {}", styles::MUTED)>
            {announced}
        </p>
    }
    .into_any();
    let back = view! {
        <p class="mt-default">
            <a class=styles::LINK href=move || ui_link(OVERVIEW_PATH, version.get())>
                "Back to the overview"
            </a>
        </p>
    }
    .into_any();

    view! {
        <Title text="Signing in" />
        <section class="mx-auto max-w-2xl">
            <h1 class=styles::PAGE_TITLE>"Signing in"</h1>
            {outcome}
            <div class="mt-default">{detail}</div>
            {back}
        </section>
    }
}

/// Completes the sign-in, or says what stopped it.
async fn finish(
    client: &FhirClient,
    version: FhirVersion,
    returned: &Returned,
    session: Session,
) -> Ending {
    // The awaits are boxed so this future holds a pointer to each read rather
    // than every state machine inlined into one.
    let offer = match Box::pin(client.sign_in_offer(version)).await {
        Ok(offer) => offer,
        Err(error) => {
            return Ending::Failed {
                message: String::from("This server could not be asked where to sign in."),
                detail: Detail::Read(Box::new(error)),
            };
        }
    };
    let Some(sign_in) = offer else {
        return Ending::Failed {
            message: String::from(
                "This server publishes no sign-in, so there is nothing to complete.",
            ),
            detail: Detail::None,
        };
    };
    match Box::pin(complete(&sign_in, &client.redirect_uri(), returned)).await {
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

/// What came back beyond the sentence, for the reader to act on.
fn detail_of(error: &SignInError) -> Detail {
    match error {
        SignInError::Refused { description, .. } => description
            .as_deref()
            .map_or(Detail::None, |said| Detail::Said(said.to_owned())),
        // RFC 6749 §5.2 is the token endpoint's own refusal, which carries no
        // `OperationOutcome`, so the body itself is the evidence and the
        // failure view is what renders it.
        SignInError::Exchange(refusal) => Detail::Read(Box::new(refusal.clone())),
        SignInError::Pkce(_)
        | SignInError::NoCode
        | SignInError::StateMismatch
        | SignInError::NoVerifier => Detail::None,
    }
}

/// The detail, drawn the way its kind is drawn everywhere else.
fn drawn(detail: &Detail) -> AnyView {
    match detail {
        Detail::None => ().into_any(),
        Detail::Said(said) => {
            let words = said.clone();
            view! {
                <div role="alert" class=styles::CALLOUT_DANGER>
                    <p>{words}</p>
                </div>
            }
            .into_any()
        }
        Detail::Read(error) => {
            let error = (**error).clone();
            view! { <Failure error=Signal::stored(error) /> }.into_any()
        }
    }
}

//! The sign-in control, and what it shows once a reader has signed in.
//!
//! The control exists only where the served version's
//! `.well-known/smart-configuration` says a sign-in can complete: an
//! authorization endpoint, a token endpoint, `S256`, and the client the
//! operator registered (<https://hl7.org/fhir/smart-app-launch/app-launch.html>).
//! A deployment that configured no issuer publishes no document, so the
//! viewer stays the read-only tool it is and draws nothing here.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::auth::Session;
use crate::auth::begin;
use crate::auth::scopes::Letter;
use crate::components::shell::SelectedVersion;
use crate::fhir::FhirClient;
use crate::fhir::smart::SignIn;
use crate::fhir::version::FhirVersion;
use crate::styles;

/// The resource types the server writes, which is what a scope can open.
///
/// The three are the definitional resources the FHIR RESTful API exposes here,
/// named as resource types and never as a code system, so a deployment serving
/// a system this viewer has never met draws the same controls.
const WRITABLE: [&str; 3] = ["CodeSystem", "ValueSet", "ConceptMap"];

/// The permissions a control can rest on, which is what a `cud` scope opens.
const LETTERS: [Letter; 3] = [Letter::Create, Letter::Update, Letter::Delete];

/// The live region the sign-in reports into.
const REPORT_ID: &str = "sign-in-report";

/// Signs a reader in, and says who is signed in and what they may change.
///
/// The discovery document is read per served version, because the sign-in is
/// against the server root the reader is looking through and the `aud` of the
/// authorization request is that root. The offer is read without a suspense
/// boundary, because there is no fallback to flash: an unread root draws no
/// control, and a refetch keeps the last value until the next one resolves
/// (<https://docs.rs/reactive_graph/0.2/reactive_graph/computed/struct.AsyncDerived.html>).
///
/// The report is a live region that is in the document whether or not it has
/// anything to say, because a region inserted along with its first message is
/// not announced (<https://www.w3.org/TR/wai-aria-1.2/#aria-live>). It is an
/// inline element, because a screen's own announcement is the paragraph the
/// accessibility pass reads and this is chrome.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn SignInControl() -> impl IntoView {
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let session = expect_context::<Session>();
    let report = RwSignal::new(String::new());

    let reader = client.clone();
    let offer = LocalResource::new(move || {
        let reader = reader.clone();
        async move { reader.sign_in_offer(version.get()).await }
    });

    // A root that offers no sign-in draws no control rather than an error, and
    // so does one that could not be read: there is nothing the reader can act
    // on here, and the screens reading the same root report the failure.
    let offered = move || {
        offer.with(|answered| {
            answered
                .as_ref()
                .and_then(|read| read.as_ref().ok().cloned())
                .flatten()
        })
    };

    let control = move || match offered() {
        None => ().into_any(),
        Some(sign_in) => {
            if session.signed_in() {
                signed_in(session, sign_in, report)
            } else {
                signed_out(client.clone(), version, sign_in, report)
            }
        }
    };

    view! {
        <div class="flex items-center gap-default">
            {control} <span id=REPORT_ID aria-live="polite" class=styles::HINT>
                {move || report.get()}
            </span>
        </div>
    }
}

/// The control a reader who is not signed in sees.
///
/// The control disables itself the moment it is pressed, because a second
/// press would draw a second verifier over the first and the redirect that
/// came back would then fail its own state check. The press is local state and
/// an event listener rather than an `Action`, because nothing renders the
/// result: the page leaves for the identity provider.
fn signed_out(
    client: FhirClient,
    version: Signal<FhirVersion>,
    sign_in: SignIn,
    report: RwSignal<String>,
) -> AnyView {
    let leaving = RwSignal::new(false);
    let start = move |_| {
        if leaving.get() {
            return;
        }
        leaving.set(true);
        let sign_in = sign_in.clone();
        let redirect_uri = client.redirect_uri();
        let audience = client.version_base(version.get());
        report.set(String::from("Opening the identity provider"));
        spawn_local(async move {
            match Box::pin(begin(&sign_in, &redirect_uri, &audience)).await {
                Ok(address) => leave_for(&address, report),
                Err(error) => {
                    leaving.set(false);
                    report.set(error.to_string());
                }
            }
        });
    };
    view! {
        <button type="button" class=styles::BUTTON disabled=move || leaving.get() on:click=start>
            "Sign in"
        </button>
    }
    .into_any()
}

/// What a signed-in reader sees: who they are, and what they may change.
fn signed_in(session: Session, sign_in: SignIn, report: RwSignal<String>) -> AnyView {
    let who = move || session.who().unwrap_or_else(|| String::from("Signed in"));
    let editable = move || {
        let open: Vec<&str> = WRITABLE
            .into_iter()
            .filter(|resource_type| {
                LETTERS
                    .into_iter()
                    .any(|letter| session.can(resource_type, letter))
            })
            .collect();
        if open.is_empty() {
            String::from("Read only: this account carries no editing permission")
        } else {
            format!("May edit: {}", open.join(", "))
        }
    };
    let out = move |_| {
        let sign_in = sign_in.clone();
        let held = session.token();
        session.release();
        report.set(String::from("Signed out"));
        spawn_local(async move {
            // RFC 7009 §2.2: the issuer answers a revocation of a token it does
            // not know as a success, so a failure here is worth reporting and
            // never worth holding the token for.
            if let Some(token) = held
                && let Err(error) = crate::fhir::revoke(&sign_in, &token).await
            {
                report.set(format!("Signed out, and the issuer said: {error}"));
            }
        });
    };
    view! {
        <span class=styles::BADGE_OK>{who}</span>
        <span class=styles::HINT>{editable}</span>
        <button type="button" class=styles::BUTTON on:click=out>
            "Sign out"
        </button>
    }
    .into_any()
}

/// Sends the browser to the identity provider.
///
/// The router owns same-origin navigation; this leaves the application for the
/// issuer, which the authorization request requires be a full browser redirect
/// (<https://www.rfc-editor.org/rfc/rfc6749#section-4.1.1>).
fn leave_for(address: &str, report: RwSignal<String>) {
    let Some(location) = web_sys::window().map(|window| window.location()) else {
        report.set(String::from("this page cannot open the identity provider"));
        return;
    };
    if location.assign(address).is_err() {
        report.set(String::from(
            "this browser refused to open the identity provider",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_writable_types_are_the_ones_the_server_writes() {
        assert_eq!(WRITABLE.len(), 3, "{WRITABLE:?}");
        for resource_type in WRITABLE {
            assert!(
                resource_type.starts_with(|letter: char| letter.is_ascii_uppercase()),
                "`{resource_type}` is a FHIR resource type"
            );
        }
    }
}

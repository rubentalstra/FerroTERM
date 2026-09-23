// SPDX-License-Identifier: BUSL-1.1
//! What a reader who signs in gets, driven through a real identity provider.
//!
//! The deployment here is the second server the harness starts: one with
//! `FERROTERM_OIDC_ISSUER` naming the stub issuer and
//! `FERROTERM_VIEWER_CLIENT_ID` naming the client the viewer presents, so the
//! served `.well-known/smart-configuration` carries everything a standalone
//! launch of a public client needs
//! (<https://hl7.org/fhir/smart-app-launch/app-launch.html>). The journeys in
//! `sign_in` drive the other server, which configures no issuer.
//!
//! Each journey chooses what it is granted by opening the issuer's `/profile`
//! address first, and tags itself so its own revocations are what it reads
//! back. The sign-in itself shows nothing and asks nothing: the issuer
//! redirects straight back with a code, because what these journeys are about
//! is what the viewer does with what comes back.

use thirtyfour::prelude::*;
use thirtyfour::stringmatch::StringMatch;

use crate::harness::Journey;
use crate::harness::SignedIn;
use crate::harness::session;
use crate::harness::signed_in;

/// The control that starts a sign-in.
const SIGN_IN: &str = "//button[normalize-space()='Sign in']";

/// The control that ends one.
const SIGN_OUT: &str = "//button[normalize-space()='Sign out']";

/// The `fhirUser` the stub issuer signs everyone in as.
const WHO: &str = "Practitioner/e2e-terminologist";

/// The shell's own mark, which proves the bundle booted.
const LOCKUP: &str = "header a[href^='/ui']";

/// Opens the issuer's profile address, which decides what the sign-in grants.
///
/// `profile` is the issuer's own vocabulary: `writer` for the full grant,
/// `reader` for the identity scopes alone. `tag` is this journey's name, which
/// keeps the revocations it reads back its own while the others run beside it.
async fn choose(journey: &Journey, deployment: &SignedIn, profile: &str, tag: &str) {
    let address = format!("{}/profile?name={profile}&tag={tag}", deployment.issuer);
    journey.reopen(&address).await;
    journey
        .text_becoming(
            By::Css("#profile"),
            StringMatch::new(profile).partial(),
            "the issuer to take the grant this journey signs in for",
        )
        .await;
}

/// Presses the sign-in control and waits for the shell to name the reader.
async fn sign_in(journey: &Journey, deployment: &SignedIn) -> String {
    journey.reopen(&format!("{}/ui", deployment.base)).await;
    journey
        .element(By::Css(LOCKUP), "the shell mark on the overview")
        .await;
    let control = journey
        .element(By::XPath(SIGN_IN), "the sign-in control")
        .await;
    control.click().await.unwrap_or_else(|error| {
        panic!("the sign-in control refused the press: {error}");
    });
    journey
        .text_becoming(
            By::XPath("//*[contains(text(), 'Practitioner/')]"),
            StringMatch::new(WHO).partial(),
            "the shell to name whoever signed in",
        )
        .await
}

/// A sign-in completes without anything being typed, and the shell says who
/// is signed in and what they may change.
///
/// The name is the `fhirUser` the identity token carries, which is the FHIR
/// identity of the person
/// (<https://hl7.org/fhir/smart-app-launch/scopes-and-launch-context.html>).
#[tokio::test]
async fn a_completed_sign_in_names_the_reader_and_what_they_may_edit() {
    let Some(deployment) = signed_in() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &deployment.base, "/ui").await;
            choose(&journey, &deployment, "writer", "names-the-reader").await;
            let named = sign_in(&journey, &deployment).await;
            assert!(named.contains(WHO), "the shell names the reader: `{named}`");

            let editable = journey
                .text_becoming(
                    By::XPath("//*[contains(text(), 'May edit')]"),
                    StringMatch::new("May edit").partial(),
                    "the shell to say what this account may change",
                )
                .await;
            for resource_type in ["CodeSystem", "ValueSet", "ConceptMap"] {
                assert!(
                    editable.contains(resource_type),
                    "{resource_type} is in the granted scopes: `{editable}`"
                );
            }
            assert_eq!(
                journey.count(By::XPath(SIGN_IN)).await,
                0,
                "the sign-in control gives way to the signed-in chrome"
            );

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}

/// A reader the issuer grants no write scope is told so, and is offered
/// nothing to change.
///
/// The viewer draws a control only where the granted scopes open it, so a
/// grant of `openid` and `fhirUser` alone leaves the chrome read-only
/// (<https://hl7.org/fhir/smart-app-launch/scopes-and-launch-context.html>).
#[tokio::test]
async fn a_reader_without_a_write_scope_is_offered_nothing_to_change() {
    let Some(deployment) = signed_in() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &deployment.base, "/ui").await;
            choose(&journey, &deployment, "reader", "no-write-scope").await;
            let named = sign_in(&journey, &deployment).await;
            assert!(named.contains(WHO), "the reader is signed in: `{named}`");

            let said = journey
                .text_becoming(
                    By::XPath("//*[contains(text(), 'Read only')]"),
                    StringMatch::new("Read only").partial(),
                    "the shell to say this account changes nothing",
                )
                .await;
            assert!(
                said.contains("no editing permission"),
                "the reader is told why: `{said}`"
            );
            assert_eq!(
                journey
                    .count(By::XPath("//*[contains(text(), 'May edit')]"))
                    .await,
                0,
                "nothing claims this account may edit"
            );

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}

/// Signing out revokes the token at the issuer and drops it here.
///
/// RFC 7009 §2.1 has the client tell the issuer the credential is finished
/// with, and the viewer holds the token in memory alone, so dropping it is the
/// whole of signing out. The issuer's own count is what proves the revocation
/// left the page.
#[tokio::test]
async fn signing_out_revokes_the_token_and_drops_it() {
    let Some(deployment) = signed_in() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &deployment.base, "/ui").await;
            // The count below is exact, so the tag is this run's own: an
            // issuer left running from an earlier run still holds the last
            // one's revocations, and nextest gives each test its own process.
            let tag = format!("revokes-on-sign-out-{}", std::process::id());
            let tag = tag.as_str();
            choose(&journey, &deployment, "writer", tag).await;
            let named = sign_in(&journey, &deployment).await;
            assert!(named.contains(WHO), "the reader is signed in: `{named}`");

            let out = journey
                .element(By::XPath(SIGN_OUT), "the sign-out control")
                .await;
            out.click().await.unwrap_or_else(|error| {
                panic!("the sign-out control refused the press: {error}");
            });
            journey
                .text_becoming(
                    By::XPath(SIGN_IN),
                    StringMatch::new("Sign in").partial(),
                    "the sign-in control to come back, which is the token being dropped",
                )
                .await;
            assert_eq!(
                journey
                    .count(By::XPath("//*[contains(text(), 'Practitioner/')]"))
                    .await,
                0,
                "nothing still names the reader who signed out"
            );

            journey.no_console_errors().await;

            journey
                .reopen(&format!("{}/revocations?tag={tag}", deployment.issuer))
                .await;
            let counted = journey
                .text_becoming(
                    By::Css("#revocations"),
                    StringMatch::new("revoked 1").partial(),
                    "the issuer to have been told the token is finished with",
                )
                .await;
            assert!(
                counted.contains(tag),
                "this journey's own count: `{counted}`"
            );
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}

// TODO(#649): the write journeys, once a screen writes through the client:
// a write refused for want of the scope, with its outcome text announced, and
// one that succeeds with `user/ValueSet.cud`. The stub issuer already serves
// the `response-only` profile the first of them needs.

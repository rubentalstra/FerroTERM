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
use crate::harness::choose;
use crate::harness::session;
use crate::harness::sign_in;
use crate::harness::signed_in;

/// The control that starts a sign-in.
const SIGN_IN: &str = "//button[normalize-space()='Sign in']";

/// The control that ends one.
const SIGN_OUT: &str = "//button[normalize-space()='Sign out']";

/// The `fhirUser` the stub issuer signs everyone in as.
const WHO: &str = "Practitioner/e2e-terminologist";

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

/// The reader bundle offers no sign-in of its own, and says where to go.
///
/// Signing in is what an editing session begins with and the editing screens
/// live in the other bundle, so the reader bundle's control is a link into it
/// rather than a sign-in that would end on the wrong page.
#[tokio::test]
async fn the_reader_bundle_sends_a_sign_in_to_the_editor_bundle() {
    let Some(deployment) = signed_in() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &deployment.base, "/ui").await;
            let offered = journey
                .element(
                    By::XPath("//a[normalize-space()='Sign in to edit']"),
                    "the way into the bundle that signs in",
                )
                .await;
            let href = offered.attr("href").await?.unwrap_or_default();
            assert!(
                href.starts_with("/ui/editor"),
                "the link opens the editor bundle: `{href}`"
            );
            assert_eq!(
                journey.count(By::XPath(SIGN_IN)).await,
                0,
                "the reader bundle starts no sign-in of its own"
            );

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}

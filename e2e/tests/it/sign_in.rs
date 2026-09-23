// SPDX-License-Identifier: BUSL-1.1
//! What the viewer offers when the deployment configured no identity provider.
//!
//! A server with no `FERROTERM_OIDC_ISSUER` serves no
//! `.well-known/smart-configuration`
//! (<https://hl7.org/fhir/smart-app-launch/conformance.html>), so the viewer
//! can complete no sign-in and offers none. The battery's server is exactly
//! that deployment, which is what these journeys drive.

use thirtyfour::prelude::*;

use crate::harness::Journey;
use crate::harness::server;
use crate::harness::session;

/// The control that starts a sign-in, wherever on the chrome it sits.
const SIGN_IN: &str = "//button[normalize-space()='Sign in']";

/// The control that ends one.
const SIGN_OUT: &str = "//button[normalize-space()='Sign out']";

/// The live region the callback screen reports into.
const OUTCOME: &str = "#sign-in-outcome";

/// The shell's own mark, which proves the bundle booted.
const LOCKUP: &str = "header a[href^='/ui']";

/// The screens a reader can reach, which between them carry every control.
const SCREENS: [&str; 6] = [
    "/ui",
    "/ui/browse",
    "/ui/expand",
    "/ui/validate",
    "/ui/valuesets",
    "/ui/about",
];

/// A deployment with no issuer offers no sign-in on any screen.
///
/// The control is what every edit control will hang off, so its absence here
/// is the whole of "without an issuer the viewer stays read only".
#[tokio::test]
async fn no_issuer_means_no_sign_in_control_on_any_screen() {
    let Some(base) = server() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &base, "/ui").await;
            for screen in SCREENS {
                journey.reopen(&format!("{base}{screen}")).await;
                // The shell is waited for first, so the count below is taken
                // on a booted page rather than on an empty body.
                journey
                    .element(By::Css(LOCKUP), &format!("the shell mark on {screen}"))
                    .await;
                assert_eq!(
                    journey.count(By::XPath(SIGN_IN)).await,
                    0,
                    "{screen} offers a sign-in the server publishes no issuer for"
                );
                assert_eq!(
                    journey.count(By::XPath(SIGN_OUT)).await,
                    0,
                    "{screen} offers a sign-out with nothing signed in"
                );
            }

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}

/// The callback address says, in a live region, that there is no sign-in here.
///
/// A reader can reach the address: it is a plain URL, and an identity provider
/// of another deployment could send someone to it. It renders what happened
/// rather than an empty screen, and announces it.
#[tokio::test]
async fn the_callback_says_there_is_no_sign_in_to_complete() {
    let Some(base) = server() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &base, "/ui/callback?code=abc&state=xyz").await;

            let said = journey
                .text_becoming(
                    By::Css(OUTCOME),
                    "no sign-in",
                    "the callback to report that this server publishes none",
                )
                .await;
            assert!(
                said.contains("nothing to complete"),
                "the reader is told what happened, not left on an empty screen: `{said}`"
            );

            let region = journey
                .element(By::Css(OUTCOME), "the callback's live region")
                .await;
            assert_eq!(
                region.attr("aria-live").await?.as_deref(),
                Some("polite"),
                "the outcome is announced rather than only drawn"
            );

            journey
                .element(By::Css("a[href^='/ui?']"), "the way back to the overview")
                .await;

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}

//! Signing in, and what the viewer holds once a reader has.
//!
//! The viewer is a public client performing a SMART standalone launch: it
//! reads the server's own discovery document, sends the reader to the
//! authorization endpoint with a PKCE challenge, and exchanges the code it
//! comes back with (<https://hl7.org/fhir/smart-app-launch/app-launch.html>).
//!
//! **The access token lives in a signal and nowhere else.** No specification
//! governs where a browser client keeps one, so this is our own design and the
//! reason is written here: `localStorage` survives the tab and is readable by
//! every script the page ever loads, and a cookie is sent on requests this
//! viewer did not make. A token in memory dies with the tab, which is the
//! shortest life a bearer credential can have while the reader is still using
//! it. Closing the tab signs out.

pub(crate) mod pkce;
pub(crate) mod scopes;

use leptos::prelude::*;

use crate::auth::scopes::Letter;
use crate::fhir::smart::Identity;
use crate::fhir::smart::SignIn;
use crate::fhir::smart::TokenAnswer;
use crate::fhir::smart::identity_of;
use crate::storage;

/// Where the one-shot PKCE verifier waits out the redirect.
const VERIFIER_KEY: &str = "ferroterm.viewer.pkce.verifier";

/// Where the one-shot CSRF state waits out the redirect.
const STATE_KEY: &str = "ferroterm.viewer.oauth.state";

/// How many milliseconds before its stated expiry a token is treated as spent.
///
/// No specification sets this: our own design, so a request is not sent with a
/// credential that expires while it is in flight.
const EXPIRY_MARGIN_MS: f64 = 5_000.0;

/// What the viewer holds for a signed-in reader.
///
/// Every field is derived from the token response (RFC 6749 §5.1) and none of
/// it is written anywhere but this value. It is `PartialEq` and not `Eq`,
/// because a deadline is a float on the page clock.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Access {
    /// The bearer credential, presented on every write.
    pub(crate) token: String,
    /// The scopes the issuer actually granted, which may be fewer than those
    /// asked for (RFC 6749 §3.3).
    pub(crate) scopes: Vec<String>,
    /// When the token stops being usable, in the page's own clock; `None` when
    /// the issuer stated no lifetime.
    pub(crate) expires_at: Option<f64>,
    /// Who is signed in, when the issuer minted an identity token.
    pub(crate) identity: Option<Identity>,
}

impl Access {
    /// Reads a token response into what the viewer holds.
    ///
    /// `now` is the page clock at the moment the answer arrived, so the stated
    /// lifetime becomes a deadline on that clock rather than on the wall.
    pub(crate) fn of(answer: &TokenAnswer, now: f64) -> Self {
        Self {
            token: answer.access_token.clone(),
            scopes: scopes::granted(answer.scope.as_deref()),
            expires_at: answer
                .expires_in
                .map(|seconds| now + f64::from(seconds) * 1_000.0 - EXPIRY_MARGIN_MS),
            identity: answer.id_token.as_deref().and_then(identity_of),
        }
    }

    /// Whether the stated lifetime has run out by `now`.
    pub(crate) fn expired(&self, now: f64) -> bool {
        self.expires_at.is_some_and(|deadline| now >= deadline)
    }

    /// The name the shell shows for whoever is signed in.
    pub(crate) fn who(&self) -> Option<&str> {
        self.identity.as_ref().and_then(Identity::display)
    }
}

/// The token the viewer holds, for every screen to consult.
///
/// It is a context newtype so the type is unambiguous, and the signal is what
/// makes a control appear the moment a reader signs in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Session(pub(crate) RwSignal<Option<Access>>);

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// A session holding no token.
    pub(crate) fn new() -> Self {
        Self(RwSignal::new(None))
    }

    /// Whether a usable token is held, read reactively.
    pub(crate) fn signed_in(&self) -> bool {
        let now = now();
        self.0
            .with(|held| held.as_ref().is_some_and(|access| !access.expired(now)))
    }

    /// The name of whoever is signed in, read reactively.
    pub(crate) fn who(&self) -> Option<String> {
        self.0.with(|held| {
            held.as_ref()
                .and_then(|access| access.who().map(str::to_owned))
        })
    }

    /// The bearer to present, read reactively; `None` when none is held or the
    /// held one has expired.
    pub(crate) fn token(&self) -> Option<String> {
        let now = now();
        self.0.with(|held| {
            held.as_ref()
                .filter(|access| !access.expired(now))
                .map(|access| access.token.clone())
        })
    }

    /// Whether the held token opens `letter` on `resource_type`.
    ///
    /// This is what decides whether a control is drawn. The server enforces
    /// the same scopes itself, so a reader who gets past this is still refused
    /// there; the point is not to offer what would be refused.
    pub(crate) fn can(&self, resource_type: &str, letter: Letter) -> bool {
        let now = now();
        self.0.with(|held| {
            held.as_ref()
                .filter(|access| !access.expired(now))
                .is_some_and(|access| scopes::can_any(&access.scopes, resource_type, letter))
        })
    }

    /// Takes the token a completed sign-in produced.
    pub(crate) fn hold(&self, access: Access) {
        self.0.set(Some(access));
    }

    /// Drops the held token, which is what signing out and a `401` both do.
    pub(crate) fn release(&self) {
        self.0.set(None);
    }
}

/// Why a sign-in did not complete.
#[derive(Clone, Debug, thiserror::Error)]
pub(crate) enum SignInError {
    /// The browser could not produce the one-shot secret.
    #[error(transparent)]
    Pkce(#[from] pkce::PkceError),
    /// The issuer refused the authorization itself (RFC 6749 §4.1.2.1).
    ///
    /// `error_description` is the issuer's own sentence and is rendered beside
    /// this, never folded into it, so the reader sees the issuer's wording
    /// unchanged.
    #[error("the identity provider refused the sign-in: {error}")]
    Refused {
        /// The `error` code the issuer sent.
        error: String,
        /// The `error_description`, when it sent one.
        description: Option<String>,
    },
    /// The redirect carried no code and no error.
    #[error("the sign-in came back without an authorization code")]
    NoCode,
    /// The redirect's `state` is not the one this browser sent.
    ///
    /// RFC 6749 §10.12 makes this the cross-site request forgery check, so a
    /// mismatch is refused rather than reported as a transient failure.
    #[error("the sign-in came back with a state this browser did not send")]
    StateMismatch,
    /// The verifier did not survive the redirect, so the exchange cannot be
    /// bound to the authorization.
    #[error("this browser no longer holds the one-time secret the sign-in started with")]
    NoVerifier,
    /// The token endpoint refused the exchange.
    #[error(transparent)]
    Exchange(#[from] crate::fhir::error::FhirError),
}

/// Starts a sign-in: draws the secrets, keeps them, and answers the address to
/// send the reader to.
///
/// The verifier and the state are kept in `sessionStorage` and nowhere else.
/// They have to survive a full page load, which a signal cannot, and
/// `sessionStorage` is per tab and cleared when it closes
/// (<https://developer.mozilla.org/en-US/docs/Web/API/Window/sessionStorage>).
/// They are one-shot secrets that die with the redirect they bind, which is
/// what makes them different from the token: a verifier is worthless once the
/// code is spent, and a state once it is compared.
///
/// # Errors
///
/// Returns [`SignInError::Pkce`] when the browser refuses the random draw or
/// the digest.
pub(crate) async fn begin(
    sign_in: &SignIn,
    redirect_uri: &str,
    audience: &str,
) -> Result<String, SignInError> {
    let secret = pkce::generate().await?;
    let state = pkce::state()?;
    storage::session_write(VERIFIER_KEY, &secret.verifier);
    storage::session_write(STATE_KEY, &state);
    Ok(sign_in.authorization_url(redirect_uri, audience, &state, &secret.challenge))
}

/// Finishes a sign-in from what the redirect came back with.
///
/// The one-shot secrets are cleared before anything else happens, so a reload
/// of the callback address cannot replay them.
///
/// # Errors
///
/// Returns the [`SignInError`] describing what the issuer sent back, or what
/// the token endpoint answered.
pub(crate) async fn complete(
    sign_in: &SignIn,
    redirect_uri: &str,
    returned: &Returned,
) -> Result<Access, SignInError> {
    let expected = storage::session_read(STATE_KEY);
    let verifier = storage::session_read(VERIFIER_KEY);
    storage::session_remove(STATE_KEY);
    storage::session_remove(VERIFIER_KEY);
    if let Some(error) = returned.error.as_deref() {
        return Err(SignInError::Refused {
            error: error.to_owned(),
            description: returned.error_description.clone(),
        });
    }
    // RFC 6749 §10.12: the value has to be the one this browser sent, and an
    // absent expectation is a mismatch rather than a pass.
    if expected.is_none() || expected.as_deref() != returned.state.as_deref() {
        return Err(SignInError::StateMismatch);
    }
    let code = returned.code.as_deref().ok_or(SignInError::NoCode)?;
    let verifier = verifier.ok_or(SignInError::NoVerifier)?;
    let answer = crate::fhir::exchange_code(sign_in, code, redirect_uri, &verifier).await?;
    Ok(Access::of(&answer, now()))
}

/// What the issuer sent back to the callback address.
///
/// RFC 6749 §4.1.2 answers `code` and `state` on success, and §4.1.2.1 answers
/// `error`, `error_description`, and `state` on a refusal.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Returned {
    /// The authorization code, on success.
    pub(crate) code: Option<String>,
    /// The `state` this browser sent with the request.
    pub(crate) state: Option<String>,
    /// The `error` code, on a refusal.
    pub(crate) error: Option<String>,
    /// The `error_description`, when the issuer sent one.
    pub(crate) error_description: Option<String>,
}

/// The page's own clock, in milliseconds.
///
/// `Performance.now()` is monotonic from the moment the page loaded, which is
/// what a lifetime should be measured against: the wall clock can move
/// (<https://developer.mozilla.org/en-US/docs/Web/API/Performance/now>). A page
/// with no `performance` reads as time zero, which makes every stated lifetime
/// look unexpired and leaves the server as the only judge, which it is anyway.
fn now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or(0.0, |clock| clock.now())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A token response granting `scope`.
    fn answered(scope: &str, expires_in: Option<u32>) -> TokenAnswer {
        TokenAnswer {
            access_token: String::from("the-token"),
            token_type: Some(String::from("Bearer")),
            expires_in,
            scope: Some(scope.to_owned()),
            id_token: None,
        }
    }

    /// A page clock reading, in milliseconds since the page loaded.
    const AT_START: f64 = 0.0;

    #[test]
    fn the_scopes_the_issuer_granted_are_the_ones_held() {
        let access = Access::of(&answered("openid user/ValueSet.cud", None), AT_START);
        assert_eq!(
            access.scopes,
            vec![String::from("openid"), String::from("user/ValueSet.cud")],
            "a subset grant is what the shell draws from"
        );
        assert_eq!(access.expires_at, None, "no stated lifetime, no deadline");
        assert!(
            !access.expired(1e12),
            "a token with no stated lifetime is the issuer's to end, not the clock's"
        );
    }

    #[test]
    fn a_stated_lifetime_becomes_a_deadline_on_the_page_clock() {
        let access = Access::of(&answered("openid", Some(3600)), AT_START);
        assert_eq!(
            access.expires_at,
            Some(3_600_000.0 - EXPIRY_MARGIN_MS),
            "an hour from the reading, less the margin a request in flight needs"
        );
        assert!(!access.expired(AT_START));
        assert!(
            access.expired(3_600_000.0),
            "the margin means the token is spent before the issuer's own deadline"
        );
    }

    #[test]
    fn a_token_whose_lifetime_has_run_out_is_spent() {
        let access = Access {
            token: String::from("t"),
            scopes: vec![String::from("user/CodeSystem.cud")],
            expires_at: Some(10.0),
            identity: None,
        };
        assert!(access.expired(10.0), "the deadline itself is already spent");
        assert!(!access.expired(9.0));
    }

    #[test]
    fn a_refusal_from_the_issuer_says_what_it_said() {
        let error = SignInError::Refused {
            error: String::from("access_denied"),
            description: Some(String::from("the user cancelled")),
        };
        assert_eq!(
            error.to_string(),
            "the identity provider refused the sign-in: access_denied",
            "the issuer's own code reaches the reader"
        );
        let SignInError::Refused { description, .. } = &error else {
            panic!("the variant is the one just built");
        };
        assert_eq!(
            description.as_deref(),
            Some("the user cancelled"),
            "the issuer's sentence travels as data, for the screen to render beside it"
        );
    }

    #[test]
    fn the_state_check_is_a_variant_and_not_a_message() {
        // RFC 6749 section 10.12: a mismatch is the cross-site request forgery
        // check firing, so the caller branches on the variant.
        let error = SignInError::StateMismatch;
        assert!(matches!(error, SignInError::StateMismatch));
    }
}

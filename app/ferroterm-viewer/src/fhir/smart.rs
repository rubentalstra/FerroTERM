//! The SMART App Launch document this server publishes, and the token flow.
//!
//! The viewer is a public client performing a standalone launch: it reads the
//! server's own `.well-known/smart-configuration`, sends the reader to the
//! authorization endpoint the document names, and exchanges the code it comes
//! back with at the token endpoint
//! (<https://hl7.org/fhir/smart-app-launch/app-launch.html>). Nothing here is
//! configured into the bundle; every address comes from the document.

use serde::Deserialize;

use crate::url::encode_query_component;

/// The `S256` code challenge method, the only one SMART admits
/// (<https://hl7.org/fhir/smart-app-launch/app-launch.html>).
pub(crate) const S256: &str = "S256";

/// The capability a server declares when it admits a client with no secret.
const CLIENT_PUBLIC: &str = "client-public";

/// The capability a server declares when it runs the standalone launch.
const LAUNCH_STANDALONE: &str = "launch-standalone";

/// The scopes the viewer asks for.
///
/// `openid` and `fhirUser` identify the person, which is what lets the shell
/// say who is signed in; the three resource scopes are the editor's, requested
/// in the `user` compartment because a person is signing in
/// (<https://hl7.org/fhir/smart-app-launch/scopes-and-launch-context.html>).
/// An identity provider that grants a subset answers with the subset in
/// `scope`, and the shell draws only what that subset opens.
pub(crate) const REQUESTED_SCOPES: [&str; 5] = [
    "openid",
    "fhirUser",
    "user/CodeSystem.cud",
    "user/ValueSet.cud",
    "user/ConceptMap.cud",
];

/// The `.well-known/smart-configuration` document, as the viewer reads it.
///
/// Only the members the sign-in needs are modelled; the server publishes more
/// (<https://hl7.org/fhir/smart-app-launch/conformance.html>).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct SmartConfiguration {
    /// `issuer`, published where the server claims `sso-openid-connect`.
    #[serde(default)]
    pub(crate) issuer: Option<String>,
    /// `authorization_endpoint`, absent from an issuer that runs no
    /// authorization code flow.
    #[serde(default)]
    pub(crate) authorization_endpoint: Option<String>,
    /// `token_endpoint`, where the code is exchanged.
    #[serde(default)]
    pub(crate) token_endpoint: Option<String>,
    /// `revocation_endpoint` (RFC 7009), when the issuer offers one.
    #[serde(default)]
    pub(crate) revocation_endpoint: Option<String>,
    /// `code_challenge_methods_supported` (RFC 7636).
    #[serde(default)]
    pub(crate) code_challenge_methods_supported: Vec<String>,
    /// `capabilities`, the SMART capability names.
    #[serde(default)]
    pub(crate) capabilities: Vec<String>,
    /// `ferroterm_viewer_client_id`: the client the operator registered for
    /// this viewer.
    ///
    /// RFC 8414 §2 admits an additional metadata parameter, and this is the
    /// one the server publishes so the bundle carries no deployment's client
    /// id.
    #[serde(default)]
    pub(crate) ferroterm_viewer_client_id: Option<String>,
}

/// Everything one authorization request needs, when the document carries it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SignIn {
    /// Where the reader is sent to authorize.
    pub(crate) authorization_endpoint: String,
    /// Where the code is exchanged for a token.
    pub(crate) token_endpoint: String,
    /// Where a token is revoked on sign-out, when the issuer offers it.
    pub(crate) revocation_endpoint: Option<String>,
    /// The client this viewer presents.
    pub(crate) client_id: String,
}

impl SmartConfiguration {
    /// What a sign-in needs, or `None` when this deployment offers none.
    ///
    /// Every condition is a requirement of the standalone launch of a public
    /// client: an authorization endpoint to send the reader to, a token
    /// endpoint to exchange the code at, `S256` for the PKCE binding, and a
    /// registered client to present
    /// (<https://hl7.org/fhir/smart-app-launch/app-launch.html>). A document
    /// missing any of them means the deployment did not set sign-in up, and
    /// the viewer stays the read-only tool it is without one.
    pub(crate) fn sign_in(&self) -> Option<SignIn> {
        let authorization_endpoint = self.authorization_endpoint.clone()?;
        let token_endpoint = self.token_endpoint.clone()?;
        let client_id = self.ferroterm_viewer_client_id.clone()?;
        if !self
            .code_challenge_methods_supported
            .iter()
            .any(|method| method == S256)
        {
            return None;
        }
        if !self.declares(CLIENT_PUBLIC) || !self.declares(LAUNCH_STANDALONE) {
            return None;
        }
        Some(SignIn {
            authorization_endpoint,
            token_endpoint,
            revocation_endpoint: self.revocation_endpoint.clone(),
            client_id,
        })
    }

    /// Whether the document claims one SMART capability.
    fn declares(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|held| held == capability)
    }
}

impl SignIn {
    /// The address the reader is sent to, with the PKCE challenge on it.
    ///
    /// The parameters are the ones the authorization request of a standalone
    /// launch carries: `response_type=code`, `client_id`, `redirect_uri`,
    /// `scope`, `state`, `aud`, `code_challenge`, and
    /// `code_challenge_method=S256`
    /// (<https://hl7.org/fhir/smart-app-launch/app-launch.html>). `aud` is the
    /// FHIR base of the served version, which is what the token will be spent
    /// against.
    pub(crate) fn authorization_url(
        &self,
        redirect_uri: &str,
        audience: &str,
        state: &str,
        challenge: &str,
    ) -> String {
        append_query(
            &self.authorization_endpoint,
            &[
                ("response_type", "code"),
                ("client_id", &self.client_id),
                ("redirect_uri", redirect_uri),
                ("scope", &REQUESTED_SCOPES.join(" ")),
                ("state", state),
                ("aud", audience),
                ("code_challenge", challenge),
                ("code_challenge_method", S256),
            ],
        )
    }

    /// The body of the token request, form-encoded.
    ///
    /// A public client sends no secret and no client authentication: the
    /// `client_id` and the `code_verifier` are what bind the request to the
    /// authorization that produced the code (RFC 6749 §4.1.3, RFC 7636 §4.5,
    /// and <https://hl7.org/fhir/smart-app-launch/app-launch.html>).
    pub(crate) fn token_request_body(
        &self,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> String {
        form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", &self.client_id),
            ("code_verifier", verifier),
        ])
    }

    /// The body of a revocation request for `token` (RFC 7009 §2.1).
    ///
    /// A public client identifies itself with `client_id` and authenticates
    /// with nothing, the same way it does at the token endpoint.
    pub(crate) fn revocation_request_body(&self, token: &str) -> String {
        form(&[
            ("token", token),
            ("token_type_hint", "access_token"),
            ("client_id", &self.client_id),
        ])
    }
}

/// The token endpoint's answer, as the viewer reads it.
///
/// `expires_in` is `u32` rather than `usize`, because WebAssembly is 32-bit
/// and a serialized width has to be fixed (RFC 6749 §5.1 makes it a number of
/// seconds).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct TokenAnswer {
    /// The credential to present (RFC 6749 §5.1).
    pub(crate) access_token: String,
    /// The type, which SMART fixes as `Bearer`.
    #[serde(default)]
    pub(crate) token_type: Option<String>,
    /// How many seconds the token is good for.
    #[serde(default)]
    pub(crate) expires_in: Option<u32>,
    /// The scopes actually granted, which may be fewer than those asked for.
    #[serde(default)]
    pub(crate) scope: Option<String>,
    /// The identity token, when `openid` was granted.
    #[serde(default)]
    pub(crate) id_token: Option<String>,
}

/// The claims the viewer reads off an identity token, for display only.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct Identity {
    /// `sub`, the issuer's own identifier for the person.
    #[serde(default)]
    pub(crate) sub: Option<String>,
    /// `fhirUser`, the URL of the resource describing the person
    /// (<https://hl7.org/fhir/smart-app-launch/scopes-and-launch-context.html>).
    #[serde(default)]
    #[serde(rename = "fhirUser")]
    pub(crate) fhir_user: Option<String>,
}

impl Identity {
    /// The name the shell shows for whoever is signed in.
    ///
    /// `fhirUser` is the FHIR-level identity and `sub` the issuer's, so the
    /// first is preferred and the second is the fallback.
    pub(crate) fn display(&self) -> Option<&str> {
        self.fhir_user.as_deref().or(self.sub.as_deref())
    }
}

/// The identity an ID token carries, read from its payload.
///
/// The signature is not checked, and does not need to be: the token came
/// straight from the token endpoint over TLS in answer to this client's own
/// request, which is the case OpenID Connect Core §3.1.3.7 exempts from
/// signature validation. Nothing here is a security decision: the server
/// validates every token it is presented with, and this is the name on a
/// label.
pub(crate) fn identity_of(id_token: &str) -> Option<Identity> {
    let mut parts = id_token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let bytes = base64url_decode(payload)?;
    serde_json::from_slice(&bytes).ok()
}

/// The bytes `text` encodes, in base64url without padding (RFC 4648 §5).
fn base64url_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len());
    let mut block = 0_u32;
    let mut filled = 0_u32;
    for letter in text.bytes() {
        let sextet = match letter {
            b'A'..=b'Z' => u32::from(letter - b'A'),
            b'a'..=b'z' => u32::from(letter - b'a') + 26,
            b'0'..=b'9' => u32::from(letter - b'0') + 52,
            b'-' => 62,
            b'_' => 63,
            // Padding is legal and carries no bits; anything else is not this
            // encoding, so the token is read as carrying no identity.
            b'=' => continue,
            _other => return None,
        };
        block = (block << 6) | sextet;
        filled += 6;
        if filled >= 8 {
            filled -= 8;
            let byte = (block >> filled) & 0xff;
            out.push(u8::try_from(byte).unwrap_or_default());
        }
    }
    Some(out)
}

/// `pairs` as an `application/x-www-form-urlencoded` body.
///
/// Every name and value is percent-encoded down to the unreserved set, which a
/// form-urlencoded reader decodes back verbatim.
fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(name, value)| {
            format!(
                "{}={}",
                encode_query_component(name),
                encode_query_component(value)
            )
        })
        .collect::<Vec<String>>()
        .join("&")
}

/// `endpoint` with `pairs` added to whatever query it already carries.
///
/// RFC 6749 §3.1: the endpoint may carry a query of its own, and it "MUST be
/// retained when adding additional query parameters".
fn append_query(endpoint: &str, pairs: &[(&str, &str)]) -> String {
    let separator = if endpoint.contains('?') { '&' } else { '?' };
    format!("{endpoint}{separator}{}", form(pairs))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document from a deployment that set sign-in up.
    fn configured() -> SmartConfiguration {
        SmartConfiguration {
            issuer: Some(String::from("https://idp.example/realms/tx")),
            authorization_endpoint: Some(String::from("https://idp.example/authorize")),
            token_endpoint: Some(String::from("https://idp.example/token")),
            revocation_endpoint: Some(String::from("https://idp.example/revoke")),
            code_challenge_methods_supported: vec![String::from("S256")],
            capabilities: vec![
                String::from("client-public"),
                String::from("launch-standalone"),
                String::from("permission-user"),
            ],
            ferroterm_viewer_client_id: Some(String::from("ferroterm-viewer")),
        }
    }

    #[test]
    fn a_document_with_everything_offers_a_sign_in() {
        let sign_in = configured().sign_in().expect("the document is complete");
        assert_eq!(sign_in.client_id, "ferroterm-viewer");
        assert_eq!(sign_in.token_endpoint, "https://idp.example/token");
        assert_eq!(
            sign_in.revocation_endpoint.as_deref(),
            Some("https://idp.example/revoke")
        );
    }

    #[test]
    fn a_document_missing_any_requirement_offers_none() {
        for (what, document) in [
            (
                "no authorization endpoint",
                SmartConfiguration {
                    authorization_endpoint: None,
                    ..configured()
                },
            ),
            (
                "no token endpoint",
                SmartConfiguration {
                    token_endpoint: None,
                    ..configured()
                },
            ),
            (
                "no registered client",
                SmartConfiguration {
                    ferroterm_viewer_client_id: None,
                    ..configured()
                },
            ),
            (
                "no S256",
                SmartConfiguration {
                    code_challenge_methods_supported: vec![String::from("plain")],
                    ..configured()
                },
            ),
            (
                "no public client",
                SmartConfiguration {
                    capabilities: vec![String::from("launch-standalone")],
                    ..configured()
                },
            ),
            (
                "no standalone launch",
                SmartConfiguration {
                    capabilities: vec![String::from("client-public")],
                    ..configured()
                },
            ),
        ] {
            assert!(
                document.sign_in().is_none(),
                "{what}: the viewer offers no sign-in it cannot complete"
            );
        }
    }

    #[test]
    fn an_empty_document_offers_no_sign_in() {
        assert!(SmartConfiguration::default().sign_in().is_none());
    }

    #[test]
    fn the_authorization_request_carries_every_parameter_the_launch_names() {
        let sign_in = configured().sign_in().expect("complete");
        let url = sign_in.authorization_url(
            "https://tx.example.org/ui/callback",
            "https://tx.example.org/r4b",
            "abc123",
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        );
        assert_eq!(
            url,
            "https://idp.example/authorize\
             ?response_type=code&client_id=ferroterm-viewer\
             &redirect_uri=https%3A%2F%2Ftx.example.org%2Fui%2Fcallback\
             &scope=openid%20fhirUser%20user%2FCodeSystem.cud%20user%2FValueSet.cud\
             %20user%2FConceptMap.cud\
             &state=abc123&aud=https%3A%2F%2Ftx.example.org%2Fr4b\
             &code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM\
             &code_challenge_method=S256",
            "every value that lands in the URL is percent-encoded"
        );
    }

    #[test]
    fn an_endpoint_with_a_query_of_its_own_keeps_it() {
        // RFC 6749 section 3.1 requires the endpoint's own query to be kept.
        let sign_in = SignIn {
            authorization_endpoint: String::from("https://idp.example/authorize?realm=tx"),
            token_endpoint: String::from("https://idp.example/token"),
            revocation_endpoint: None,
            client_id: String::from("v"),
        };
        let url = sign_in.authorization_url("https://tx.example.org/ui/callback", "a", "s", "c");
        assert!(url.starts_with("https://idp.example/authorize?realm=tx&response_type=code"));
        assert_eq!(url.matches('?').count(), 1, "{url}");
    }

    #[test]
    fn the_token_request_is_a_public_client_exchange() {
        let sign_in = configured().sign_in().expect("complete");
        assert_eq!(
            sign_in.token_request_body(
                "the/code",
                "https://tx.example.org/ui/callback",
                "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
            ),
            "grant_type=authorization_code&code=the%2Fcode\
             &redirect_uri=https%3A%2F%2Ftx.example.org%2Fui%2Fcallback\
             &client_id=ferroterm-viewer\
             &code_verifier=dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
            "no client secret travels, because a public client holds none"
        );
    }

    #[test]
    fn the_revocation_request_names_the_token_and_the_client() {
        let sign_in = configured().sign_in().expect("complete");
        assert_eq!(
            sign_in.revocation_request_body("abc"),
            "token=abc&token_type_hint=access_token&client_id=ferroterm-viewer"
        );
    }

    #[test]
    fn the_identity_token_yields_the_name_the_shell_shows() {
        // A payload with both claims, base64url without padding.
        let payload = "eyJzdWIiOiIxMjMiLCJmaGlyVXNlciI6Imh0dHBzOi8vdHguZXhhbXBsZS5vcmcvUHJhY3RpdGlvbmVyLzcifQ";
        let identity = identity_of(&format!("header.{payload}.signature"))
            .expect("the payload is readable JSON");
        assert_eq!(
            identity.display(),
            Some("https://tx.example.org/Practitioner/7"),
            "fhirUser is the FHIR-level identity and wins over sub"
        );
        assert_eq!(identity.sub.as_deref(), Some("123"));
    }

    #[test]
    fn a_token_with_only_a_subject_still_names_the_reader() {
        // {"sub":"ruben"}
        let identity =
            identity_of("h.eyJzdWIiOiJydWJlbiJ9.s").expect("the payload is readable JSON");
        assert_eq!(identity.display(), Some("ruben"));
    }

    #[test]
    fn an_unreadable_identity_token_names_nobody_rather_than_failing() {
        for token in ["", "one-part", "h.not-base64!!.s", "h..s"] {
            assert!(
                identity_of(token).map(|read| read.display().map(str::to_owned))
                    != Some(Some(String::new())),
                "`{token}` names nobody"
            );
        }
        assert!(identity_of("only-one-part").is_none());
    }

    #[test]
    fn the_scopes_asked_for_are_the_editor_scopes_of_a_person() {
        assert!(REQUESTED_SCOPES.contains(&"openid"));
        for resource_type in ["CodeSystem", "ValueSet", "ConceptMap"] {
            assert!(
                REQUESTED_SCOPES.contains(&format!("user/{resource_type}.cud").as_str()),
                "{resource_type} is one of the three the server writes"
            );
        }
        assert!(
            !REQUESTED_SCOPES
                .iter()
                .any(|scope| scope.starts_with("system/")),
            "the viewer signs a person in, never a client in its own right"
        );
    }
}

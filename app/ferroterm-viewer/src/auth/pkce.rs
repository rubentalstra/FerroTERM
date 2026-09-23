//! Proof Key for Code Exchange: the one-shot secret that binds the redirect.
//!
//! A public client holds no secret, so the authorization code alone would be
//! enough for anyone who intercepted the redirect. PKCE closes that: the client
//! keeps a random verifier, sends only its SHA-256 digest with the
//! authorization request, and presents the verifier itself at the token
//! endpoint (<https://www.rfc-editor.org/rfc/rfc7636>).

use wasm_bindgen::JsCast;

/// How many random bytes one verifier is drawn from.
///
/// RFC 7636 §4.1 asks for at least 256 bits of entropy and bounds the encoded
/// verifier at 43 to 128 characters; 32 bytes encode to exactly 43.
const VERIFIER_BYTES: usize = 32;

/// The digest the `S256` challenge method names (RFC 7636 §4.2).
const SHA_256: &str = "SHA-256";

/// The alphabet base64url writes, without padding (RFC 4648 §5).
const BASE64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Why the browser could not produce a PKCE secret.
#[derive(Clone, Debug, thiserror::Error)]
pub(crate) enum PkceError {
    /// The page has no window, so there is no `crypto` to ask.
    #[error("this page has no browser crypto to draw a one-time secret from")]
    Unavailable,
    /// `crypto.getRandomValues` or `crypto.subtle.digest` refused.
    #[error("the browser refused to produce a one-time secret: {message}")]
    Refused {
        /// What the browser reported.
        message: String,
    },
}

/// One verifier and the challenge derived from it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Pkce {
    /// The secret kept until the token request (RFC 7636 §4.1).
    pub(crate) verifier: String,
    /// The `S256` challenge sent with the authorization request (§4.2).
    pub(crate) challenge: String,
}

/// Draws a verifier and derives its `S256` challenge from the browser's crypto.
///
/// # Errors
///
/// Returns [`PkceError`] when the page has no `crypto`, or when the browser
/// refuses the random draw or the digest.
pub(crate) async fn generate() -> Result<Pkce, PkceError> {
    let crypto = web_sys::window()
        .ok_or(PkceError::Unavailable)?
        .crypto()
        .map_err(|error| refused(&error))?;
    let mut bytes = [0_u8; VERIFIER_BYTES];
    crypto
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(|error| refused(&error))?;
    let verifier = verifier_from(&bytes);
    let promise = crypto
        .subtle()
        .digest_with_str_and_u8_array(SHA_256, verifier.as_bytes())
        .map_err(|error| refused(&error))?;
    let digest = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|error| refused(&error))?;
    let buffer = digest
        .dyn_ref::<js_sys::ArrayBuffer>()
        .ok_or_else(|| PkceError::Refused {
            message: String::from("the digest did not come back as bytes"),
        })?;
    let challenge = base64url(&js_sys::Uint8Array::new(buffer).to_vec());
    Ok(Pkce {
        verifier,
        challenge,
    })
}

/// Draws the `state` that binds the redirect to this browser.
///
/// RFC 6749 §10.12 asks for a value "that cannot be guessed", which is the
/// same draw the verifier takes.
///
/// # Errors
///
/// Returns [`PkceError`] when the page has no `crypto`, or the browser refuses
/// the draw.
pub(crate) fn state() -> Result<String, PkceError> {
    let crypto = web_sys::window()
        .ok_or(PkceError::Unavailable)?
        .crypto()
        .map_err(|error| refused(&error))?;
    let mut bytes = [0_u8; VERIFIER_BYTES];
    crypto
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(|error| refused(&error))?;
    Ok(base64url(&bytes))
}

/// The browser's own report of a refusal, as a message.
fn refused(error: &wasm_bindgen::JsValue) -> PkceError {
    PkceError::Refused {
        message: error
            .as_string()
            .unwrap_or_else(|| String::from("the browser gave no reason")),
    }
}

/// The verifier `bytes` encode to.
///
/// RFC 7636 §4.1 fixes the alphabet as the unreserved characters of RFC 3986,
/// and base64url is a subset of it, which is what the same section recommends
/// for a verifier drawn from random bytes.
fn verifier_from(bytes: &[u8]) -> String {
    base64url(bytes)
}

/// `bytes` in base64url without padding (RFC 4648 §5).
///
/// The encoder is written out because the viewer carries no base64 dependency,
/// and one more crate on a client path is weighed against the bundle it adds.
fn base64url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let mut block = 0_u32;
        for (index, byte) in chunk.iter().enumerate() {
            // Each byte fills one of the three octets of a 24-bit group, most
            // significant first (RFC 4648 §4).
            block |= u32::from(*byte) << (16 - 8 * index);
        }
        // A group of n input bytes yields n + 1 output characters, which is
        // what dropping the padding means (RFC 4648 §5).
        for index in 0..=chunk.len() {
            let sextet = (block >> (18 - 6 * index)) & 0b0011_1111;
            let position = usize::try_from(sextet).unwrap_or_default();
            if let Some(letter) = BASE64URL.get(position) {
                out.push(char::from(*letter));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How many characters a verifier may carry, as RFC 7636 §4.1 bounds it.
    const VERIFIER_RANGE: std::ops::RangeInclusive<usize> = 43..=128;

    /// The 32-byte SHA-256 digest of the RFC 7636 appendix B verifier.
    ///
    /// The appendix gives the verifier and the challenge it must produce; the
    /// digest between them is the SHA-256 of the verifier's ASCII octets, which
    /// is what the S256 method is defined as (RFC 7636 §4.2).
    const APPENDIX_B_DIGEST: [u8; 32] = [
        19, 211, 30, 150, 26, 26, 216, 236, 47, 22, 177, 12, 76, 152, 46, 8, 118, 168, 120, 173,
        109, 241, 68, 86, 110, 225, 137, 74, 203, 112, 249, 195,
    ];

    /// The verifier of RFC 7636 appendix B.
    const APPENDIX_B_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

    /// The challenge of RFC 7636 appendix B.
    const APPENDIX_B_CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

    #[test]
    fn the_appendix_b_vector_produces_the_challenge_the_rfc_publishes() {
        assert_eq!(
            base64url(&APPENDIX_B_DIGEST),
            APPENDIX_B_CHALLENGE,
            "the S256 challenge is the base64url of the verifier's SHA-256 (RFC 7636 section 4.2)"
        );
    }

    #[test]
    fn a_verifier_drawn_from_thirty_two_bytes_is_forty_three_characters() {
        let verifier = verifier_from(&[0_u8; VERIFIER_BYTES]);
        assert_eq!(
            verifier.len(),
            43,
            "RFC 7636 section 4.1 bounds a verifier at 43 to 128 characters"
        );
        assert!(VERIFIER_RANGE.contains(&verifier.len()));
    }

    #[test]
    fn every_character_of_a_verifier_is_unreserved() {
        // Every byte value reaches the encoder, so the whole alphabet is
        // exercised rather than the part a zero-filled buffer produces.
        let bytes: Vec<u8> = (0..=255_u8).collect();
        let verifier = verifier_from(&bytes);
        for character in verifier.chars() {
            assert!(
                character.is_ascii_alphanumeric() || matches!(character, '-' | '.' | '_' | '~'),
                "`{character}` is outside the unreserved set RFC 7636 section 4.1 fixes"
            );
        }
    }

    #[test]
    fn the_appendix_b_verifier_is_itself_a_legal_verifier() {
        assert!(VERIFIER_RANGE.contains(&APPENDIX_B_VERIFIER.len()));
        for character in APPENDIX_B_VERIFIER.chars() {
            assert!(character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
        }
    }

    #[test]
    fn the_encoder_drops_the_padding_at_every_remainder() {
        // RFC 4648 section 4: a 24-bit group is four characters, and a partial
        // group is one character per input byte plus one.
        assert_eq!(base64url(b""), "");
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url(b"foob"), "Zm9vYg");
        assert_eq!(base64url(b"fooba"), "Zm9vYmE");
        assert_eq!(base64url(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn the_two_url_safe_characters_replace_the_standard_ones() {
        // RFC 4648 section 5 swaps `+` and `/` for `-` and `_`, which is what
        // keeps a challenge legal in a query without encoding.
        assert_eq!(base64url(&[251, 255]), "-_8");
    }
}

//! The issuer's signing keys, read at start and refreshed on an unknown `kid`.
//!
//! A JWKS is a set of public keys, each optionally named by a `kid` that the
//! token's header repeats (RFC 7517 §4.5, RFC 7515 §4.1.4). An issuer that
//! rotates its keys publishes the new one before it signs with it, so a token
//! naming a key this server has not seen is the signal to read the set again.

use std::sync::{Arc, PoisonError, RwLock};
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::{Jwk, JwkSet, KeyOperations, PublicKeyUse};
use jsonwebtoken::{Algorithm, DecodingKey};

use crate::smart::discovery::{FetchError, Http};

/// How long one on-demand refresh holds off the next.
///
/// No specification governs the interval: our own design, so a stream of
/// tokens naming keys the issuer never published cannot turn into a stream of
/// requests to the issuer.
pub const REFRESH_COOLDOWN: Duration = Duration::from_secs(60);

/// A key the token cannot be verified with.
#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    /// The set carries no key of that `kid`, and a refresh did not add one.
    #[error("the issuer publishes no signing key `{0}`")]
    UnknownKeyId(String),
    /// The token names no `kid` and the set carries no single usable key.
    #[error("the issuer publishes no single key usable for `{0:?}`")]
    NoUsableKey(Algorithm),
    /// The token names no `kid` and several keys could verify it.
    #[error("the issuer publishes {0} keys usable for the token, and it names none")]
    AmbiguousKey(usize),
    /// The selected JWK does not read as a verification key.
    #[error("the issuer's signing key does not read")]
    UnusableJwk {
        /// The cause.
        #[source]
        source: jsonwebtoken::errors::Error,
    },
    /// The set does not fetch.
    #[error("the issuer's key set does not fetch")]
    Fetch {
        /// The cause.
        #[source]
        source: Box<FetchError>,
    },
}

/// The issuer's key set, cached and refreshed on demand.
#[derive(Debug)]
pub struct Keys {
    /// Where the set is published.
    uri: String,
    /// The set as of the last read.
    cached: RwLock<Arc<JwkSet>>,
    /// When an unknown `kid` last caused a refresh; `None` until the first one.
    refreshed: RwLock<Option<Instant>>,
    /// Held for the length of one refresh, so tokens arriving together read
    /// the set once.
    refreshing: tokio::sync::Mutex<()>,
}

impl Keys {
    /// Reads the set at `uri` once and holds it.
    ///
    /// # Errors
    ///
    /// Returns [`KeyError::Fetch`] when the set does not arrive or does not
    /// parse, which is what refuses the start.
    pub async fn read(http: &Http, uri: &str) -> Result<Self, KeyError> {
        let set: JwkSet = http.json(uri).await.map_err(|source| KeyError::Fetch {
            source: Box::new(source),
        })?;
        Ok(Self {
            uri: uri.to_owned(),
            cached: RwLock::new(Arc::new(set)),
            refreshed: RwLock::new(None),
            refreshing: tokio::sync::Mutex::new(()),
        })
    }

    /// The verification key for a token naming `kid` and signed with `algorithm`.
    ///
    /// An unknown `kid` reads the set again, at most once per
    /// [`REFRESH_COOLDOWN`].
    ///
    /// # Errors
    ///
    /// Returns [`KeyError`] when no key matches after the refresh, when the
    /// token names no `kid` and the set is ambiguous, or when the set does not
    /// fetch.
    pub async fn key(
        &self,
        http: &Http,
        kid: Option<&str>,
        algorithm: Algorithm,
    ) -> Result<DecodingKey, KeyError> {
        let first = select(&self.snapshot(), kid, algorithm);
        match first {
            Ok(key) => return Ok(key),
            Err(KeyError::UnknownKeyId(_)) => {}
            Err(other) => return Err(other),
        }
        let refreshing = self.refreshing.lock().await;
        // Another token may have refreshed the set while this one waited.
        if let Ok(key) = select(&self.snapshot(), kid, algorithm) {
            drop(refreshing);
            return Ok(key);
        }
        if !self.may_refresh() {
            drop(refreshing);
            return Err(KeyError::UnknownKeyId(kid.unwrap_or_default().to_owned()));
        }
        let fetched: Result<JwkSet, FetchError> = http.json(&self.uri).await;
        self.mark_refreshed();
        let set = fetched.map_err(|source| KeyError::Fetch {
            source: Box::new(source),
        })?;
        let set = Arc::new(set);
        {
            let mut cached = self.cached.write().unwrap_or_else(PoisonError::into_inner);
            *cached = Arc::clone(&set);
        }
        drop(refreshing);
        select(&set, kid, algorithm)
    }

    /// The set as of now.
    fn snapshot(&self) -> Arc<JwkSet> {
        Arc::clone(&self.cached.read().unwrap_or_else(PoisonError::into_inner))
    }

    /// Whether the cooldown admits another refresh.
    fn may_refresh(&self) -> bool {
        self.refreshed
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .is_none_or(|at| at.elapsed() >= REFRESH_COOLDOWN)
    }

    /// Records that a refresh happened now.
    fn mark_refreshed(&self) {
        let mut refreshed = self
            .refreshed
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        *refreshed = Some(Instant::now());
    }
}

/// The key of `set` that verifies a token naming `kid` and signed with
/// `algorithm`.
///
/// A `kid` names the key outright (RFC 7515 §4.1.4). Without one the candidates
/// are narrowed by `use`, `key_ops`, and `alg` (RFC 7517 §4.2 to §4.4), and an
/// ambiguous remainder is refused: taking the first key would let a rotation
/// decide which key verifies a write.
fn select(set: &JwkSet, kid: Option<&str>, algorithm: Algorithm) -> Result<DecodingKey, KeyError> {
    let jwk = if let Some(kid) = kid {
        let named = set
            .find(kid)
            .ok_or_else(|| KeyError::UnknownKeyId(kid.to_owned()))?;
        if !usable(named, algorithm) {
            return Err(KeyError::NoUsableKey(algorithm));
        }
        named
    } else {
        let mut candidates = set.keys.iter().filter(|jwk| usable(jwk, algorithm));
        let first = candidates.next().ok_or(KeyError::NoUsableKey(algorithm))?;
        let rest = candidates.count();
        if rest > 0 {
            return Err(KeyError::AmbiguousKey(rest.saturating_add(1)));
        }
        first
    };
    DecodingKey::from_jwk(jwk).map_err(|source| KeyError::UnusableJwk { source })
}

/// Whether `jwk` may verify a signature made with `algorithm`.
fn usable(jwk: &Jwk, algorithm: Algorithm) -> bool {
    let common = &jwk.common;
    let for_signatures = common
        .public_key_use
        .as_ref()
        .is_none_or(|purpose| matches!(purpose, PublicKeyUse::Signature));
    let verifies = common.key_operations.as_ref().is_none_or(|operations| {
        operations
            .iter()
            .any(|operation| matches!(operation, KeyOperations::Verify))
    });
    let same_algorithm = common.key_algorithm.as_ref().is_none_or(|declared| {
        declared
            .to_string()
            .eq_ignore_ascii_case(&format!("{algorithm:?}"))
    });
    for_signatures && verifies && same_algorithm
}

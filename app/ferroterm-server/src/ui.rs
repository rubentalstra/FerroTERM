//! The viewer bundle this binary carries, served under `/ui`.
//!
//! No FHIR specification governs a terminology server's user interface: our
//! own design. The bundle is a table of files compiled into the binary, so a
//! request path never reaches the filesystem and there is nothing to traverse
//! out of; a path the table does not hold answers the single-page document,
//! which is how a client-side route deep-links.
//!
//! The table is empty unless the `ui` feature was on and the viewer's `dist/`
//! existed when this crate was built, and the server mounts no route over an
//! empty table.

use axum::Router;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use http::StatusCode;
use http::header::{CACHE_CONTROL, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};

/// One file of the viewer bundle, as the binary carries it.
#[derive(Debug, Clone, Copy)]
pub struct Asset {
    /// The path under `/ui/`, `/`-separated and without a leading slash.
    pub path: &'static str,
    /// The bytes the build wrote.
    pub bytes: &'static [u8],
}

/// The document every path the bundle does not hold falls back to.
pub const INDEX: &str = "index.html";

/// The path the viewer is mounted at, with its trailing slash.
pub const MOUNT: &str = "/ui/";

/// The media type served for an extension [`MEDIA_TYPES`] does not name.
const OCTET_STREAM: &str = "application/octet-stream";

/// The media type of each extension the bundle carries.
///
/// `application/wasm` is the registered type
/// (<https://www.iana.org/assignments/media-types/application/wasm>) and a
/// browser refuses to stream-compile a module served as anything else;
/// `text/javascript` is the type RFC 9239 §6 settles on.
const MEDIA_TYPES: [(&str, &str); 16] = [
    ("css", "text/css; charset=utf-8"),
    ("html", "text/html; charset=utf-8"),
    ("ico", "image/vnd.microsoft.icon"),
    ("jpeg", "image/jpeg"),
    ("jpg", "image/jpeg"),
    ("js", "text/javascript; charset=utf-8"),
    ("json", "application/json"),
    ("map", "application/json"),
    ("png", "image/png"),
    ("svg", "image/svg+xml"),
    ("ttf", "font/ttf"),
    ("txt", "text/plain; charset=utf-8"),
    ("wasm", "application/wasm"),
    ("webp", "image/webp"),
    ("woff", "font/woff"),
    ("woff2", "font/woff2"),
];

/// A year of caching, for a file whose name carries its own content hash.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// No reuse without revalidation, for a file whose name is stable.
const REVALIDATE: &str = "no-cache";

/// The routes the viewer adds: the document at [`MOUNT`], every asset under
/// it, and `/ui` itself redirecting onto the mount.
///
/// The caller merges these after the request-log middleware, so an asset is
/// neither logged nor timed and the `/metrics` histograms keep describing the
/// terminology operations.
pub fn router<S>(bundle: &'static [Asset]) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/ui", get(|| async { Redirect::temporary(MOUNT) }))
        .route("/ui/", get(move || async move { document(bundle) }))
        .route(
            "/ui/{*path}",
            get(
                move |axum::extract::Path(path): axum::extract::Path<String>| async move {
                    asset(bundle, &path)
                },
            ),
        )
}

/// `GET /`: the redirect onto the viewer, for a reader who typed the host.
///
/// The redirect is temporary because the viewer is a switch: a permanent one
/// would outlive `FERROTERM_UI=off` in every browser cache that saw it.
pub async fn root() -> Redirect {
    Redirect::temporary(MOUNT)
}

/// The response for `path` under the mount: the asset the bundle holds at it,
/// or the document, so a client-side route deep-links.
fn asset(bundle: &'static [Asset], path: &str) -> Response {
    bundle
        .iter()
        .find(|asset| asset.path == path)
        .map_or_else(|| document(bundle), served)
}

/// The response for the single-page document, or `404` when the bundle holds
/// none.
fn document(bundle: &'static [Asset]) -> Response {
    bundle
        .iter()
        .find(|asset| asset.path == INDEX)
        .map_or_else(|| StatusCode::NOT_FOUND.into_response(), served)
}

/// `asset`, with its media type, its cache policy, and no content sniffing.
fn served(asset: &Asset) -> Response {
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, media_type(asset.path)),
            (CACHE_CONTROL, cache_control(asset.path)),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        asset.bytes,
    )
        .into_response()
}

/// The media type of the file at `path`.
fn media_type(path: &str) -> &'static str {
    let extension = path.rsplit_once('.').map_or("", |(_, last)| last);
    MEDIA_TYPES
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(extension))
        .map_or(OCTET_STREAM, |(_, media)| *media)
}

/// The cache policy of the file at `path`.
///
/// Only a name carrying its own content hash is cached immutably, so the
/// header states what the build guarantees.
fn cache_control(path: &str) -> &'static str {
    if hashed(path) { IMMUTABLE } else { REVALIDATE }
}

/// Whether the file name of `path` carries a content hash.
///
/// Trunk's `filehash = true` writes the hash into the file name as its own
/// run of lowercase hexadecimal characters
/// (<https://trunkrs.dev/configuration/>), which is what makes an immutable
/// cache header true rather than a promise.
fn hashed(path: &str) -> bool {
    let name = path.rsplit_once('/').map_or(path, |(_, last)| last);
    name.split(['-', '_', '.'])
        .any(|token| token.len() >= 8 && token.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')))
}

// The build script writes the table from the viewer's `dist/`, one
// `include_bytes!` per file, so the bundle rides inside the binary.
#[cfg(feature = "ui")]
include!(concat!(env!("OUT_DIR"), "/ui_bundle.rs"));

/// The bundle compiled into this binary, empty because the `ui` feature is
/// off.
#[cfg(not(feature = "ui"))]
pub const BUNDLE: &[Asset] = &[];

#[cfg(test)]
mod tests {
    use super::{IMMUTABLE, OCTET_STREAM, REVALIDATE, cache_control, hashed, media_type};

    #[test]
    fn every_asset_carries_the_media_type_its_extension_names() {
        assert_eq!(media_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(
            media_type("ferroterm-viewer-0123456789abcdef_bg.wasm"),
            "application/wasm"
        );
        assert_eq!(
            media_type("ferroterm-viewer-0123456789abcdef.js"),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            media_type("tailwind-0123456789abcdef.css"),
            "text/css; charset=utf-8"
        );
        assert_eq!(media_type("robots.txt"), "text/plain; charset=utf-8");
        assert_eq!(media_type("noextension"), OCTET_STREAM);
        assert_eq!(media_type("something.unknown"), OCTET_STREAM);
    }

    #[test]
    fn only_a_content_hashed_name_is_cached_immutably() {
        assert!(hashed("ferroterm-viewer-0123456789abcdef_bg.wasm"));
        assert!(hashed("tailwind-0123456789abcdef.css"));
        assert!(!hashed("index.html"));
        assert!(!hashed("robots.txt"));
        assert!(!hashed("snippets/logo.svg"));
        assert_eq!(cache_control("index.html"), REVALIDATE);
        assert_eq!(
            cache_control("ferroterm-viewer-0123456789abcdef.js"),
            IMMUTABLE
        );
    }
}

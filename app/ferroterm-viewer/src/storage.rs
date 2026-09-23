//! Per-viewer values kept in the browser's storage.
//!
//! Every preference the viewer holds lives here and nowhere else: the server
//! stores nothing for a reader, so a preference belongs to the browser that
//! set it. Preferences use `localStorage`, which outlives the tab. The second
//! half of this module is `sessionStorage`, which does not, and holds only the
//! two one-shot secrets a sign-in redirect has to carry across a page load
//! (<https://developer.mozilla.org/en-US/docs/Web/API/Window/sessionStorage>).

/// Reads the value stored under `key`, if there is one.
pub(crate) fn read(key: &str) -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()
        .flatten()?
        .get_item(key)
        .ok()
        .flatten()
}

/// Stores `value` under `key`, and does nothing when storage is unavailable.
///
/// A browser configured to block site data throws on the access itself
/// (<https://developer.mozilla.org/en-US/docs/Web/API/Window/localStorage>).
pub(crate) fn write(key: &str, value: &str) {
    // NOTE: A blocked localStorage means the preference is legitimately absent
    // for this reader, so the failure is not propagated as an error.
    if let Some(Some(storage)) =
        web_sys::window().map(|window| window.local_storage().ok().flatten())
        && storage.set_item(key, value).is_err()
    {
        leptos::logging::warn!("this browser refused to store the viewer preference {key}");
    }
}

/// The `sessionStorage` of this tab, when the browser opens it.
fn session() -> Option<web_sys::Storage> {
    web_sys::window()?.session_storage().ok().flatten()
}

/// Reads the per-tab value stored under `key`, if there is one.
pub(crate) fn session_read(key: &str) -> Option<String> {
    session()?.get_item(key).ok().flatten()
}

/// Stores `value` under `key` for this tab only.
///
/// A browser configured to block site data throws on the access itself, and a
/// sign-in that cannot keep its verifier fails later with a reason, so nothing
/// is propagated from here.
pub(crate) fn session_write(key: &str, value: &str) {
    if let Some(storage) = session()
        && storage.set_item(key, value).is_err()
    {
        leptos::logging::warn!("this browser refused to hold the sign-in secret {key}");
    }
}

/// Forgets the per-tab value under `key`.
pub(crate) fn session_remove(key: &str) {
    if let Some(storage) = session()
        && storage.remove_item(key).is_err()
    {
        leptos::logging::warn!("this browser refused to forget the sign-in secret {key}");
    }
}

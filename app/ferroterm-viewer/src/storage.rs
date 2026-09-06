//! Per-viewer values kept in the browser's `localStorage`.
//!
//! Every preference the viewer holds lives here and nowhere else: the server
//! stores nothing for a reader, so a preference belongs to the browser that
//! set it.

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

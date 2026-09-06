//! The routed screens.

// The diagnostic lands on macro output rather than on an item written here,
// so the expectation covers the module tree.
#![expect(
    clippy::same_name_method,
    reason = "leptos::component derives a TypedBuilder whose `builder` shadows a trait method"
)]

pub(crate) mod home;
pub(crate) mod not_found;
pub(crate) mod settings;

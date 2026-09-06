//! The shell chrome every screen renders inside.

// The diagnostic lands on macro output rather than on an item written here,
// so the expectation covers the module tree.
#![expect(
    clippy::same_name_method,
    reason = "leptos::component derives a TypedBuilder whose `builder` shadows a trait method"
)]

pub(crate) mod failure;
pub(crate) mod health;
pub(crate) mod shell;
pub(crate) mod theme_toggle;
pub(crate) mod version_switcher;

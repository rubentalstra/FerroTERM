// SPDX-License-Identifier: BUSL-1.1
//! The class strings more than one screen paints a control with.
//!
//! A palette that lives in one place is measured once. The submit button and
//! the selected version both failed the WCAG 2.2 AA contrast bar in the dark
//! theme while every copy of their class string read the same
//! (<https://www.w3.org/TR/WCAG22/#contrast-minimum>), so the strings live
//! here and the accessibility pass measures what every screen draws.

/// The control that submits a runner's form.
///
/// White on `brand-700` is 5.5:1 in the light theme. The dark theme inverts
/// the pairing rather than darkening it further: `brand-400` under
/// `slate-900` text is 9.6:1 as text and 9.6:1 against the page behind it,
/// where white on `brand-600` was 3.7:1 and failed both bars at once
/// (<https://www.w3.org/TR/WCAG22/#non-text-contrast>).
pub(crate) const SUBMIT: &str = "inline-flex items-center gap-1.5 rounded bg-brand-700 px-3 py-1.5 \
                                 text-sm font-medium text-white hover:bg-brand-900 \
                                 dark:bg-brand-400 dark:text-slate-900 dark:hover:bg-brand-300";

// SPDX-License-Identifier: BUSL-1.1
//! The viewer's design system, as the class strings a screen paints with.
//!
//! A screen names a role and a step from this file and never a palette entry,
//! a raw size, or a `dark:` colour. Two things follow. The palette is measured
//! once, which is how the WCAG 2.2 AA pass in `e2e/tests/it/accessibility.rs`
//! can cover every pairing the viewer draws
//! (<https://www.w3.org/TR/WCAG22/#contrast-minimum>). And a theme is one
//! block of `style/tailwind.css` rather than a prefix on every element.
//!
//! The roles, the six type steps, and the density variables are defined in
//! that stylesheet; this file is the vocabulary built on them.

/// The page ground and the text on it.
pub(crate) const PAGE: &str = "bg-surface text-fg";

/// A raised surface: a card, a pane, a panel.
pub(crate) const PANEL: &str = "rounded-lg border border-line bg-raised";

/// The title of a screen. One per screen, and the loudest thing on it.
pub(crate) const PAGE_TITLE: &str = "text-display font-semibold text-fg";

/// The title of a section within a screen.
pub(crate) const SECTION_TITLE: &str = "text-title font-medium text-fg";

/// The sentence under a title that says what a screen or a section is for.
pub(crate) const LEAD: &str = "mt-1 text-body text-muted";

/// A label over a group, in small caps, for a thing that is not a heading.
pub(crate) const EYEBROW: &str = "text-micro font-semibold tracking-wide text-muted uppercase";

/// Body prose that supports something else.
pub(crate) const MUTED: &str = "text-small text-muted";

/// The quietest readable text: a hint under a control, a note beside a figure.
pub(crate) const HINT: &str = "text-small text-faint";

/// An identifier, a code, or a canonical.
pub(crate) const CODE: &str = "font-mono text-small text-fg wrap-break-word";

/// An identifier that supports something else.
pub(crate) const CODE_MUTED: &str = "font-mono text-small text-muted wrap-break-word";

/// A table that compares rows.
pub(crate) const TABLE: &str = "w-full border-collapse text-body";

/// A table's header cell.
pub(crate) const TH: &str = "row-y border-b border-line-strong px-3 text-left text-small \
                             font-semibold whitespace-nowrap text-muted";

/// A table's cell.
pub(crate) const TD: &str = "row-y border-b border-line px-3 align-top";

/// A table cell whose content is one token and must not wrap.
pub(crate) const TD_TIGHT: &str = "row-y border-b border-line px-3 align-top whitespace-nowrap";

/// The control that submits a form.
pub(crate) const SUBMIT: &str = "inline-flex items-center gap-1.5 rounded-md bg-accent px-3 py-1.5 \
                                 text-small font-medium text-accent-fg";

/// A control that acts without submitting.
pub(crate) const BUTTON: &str = "inline-flex items-center gap-1.5 rounded-md border border-line \
                                 px-2.5 py-1 text-small font-medium text-fg hover:bg-inset";

/// A control with no chrome until it is pointed at.
pub(crate) const BUTTON_QUIET: &str = "inline-flex items-center gap-1.5 rounded-md px-2 py-1 \
                                       text-small font-medium text-muted hover:bg-inset \
                                       hover:text-fg";

/// A text input.
pub(crate) const INPUT: &str = "w-full rounded-md border border-line-strong bg-raised px-2.5 \
                                py-1.5 text-body text-fg";

/// The label over an input.
pub(crate) const LABEL: &str = "text-small font-medium text-fg";

/// A standing mark: a state a resource or a version is in.
pub(crate) const BADGE: &str = "inline-flex items-center gap-1 rounded-full bg-inset px-2 py-0.5 \
                                text-micro font-medium text-muted";

/// A standing mark for something that is working.
pub(crate) const BADGE_OK: &str = "inline-flex items-center gap-1 rounded-full bg-ok-soft px-2 \
                                   py-0.5 text-micro font-medium text-ok-soft-fg";

/// A standing mark for something that failed.
pub(crate) const BADGE_DANGER: &str = "inline-flex items-center gap-1 rounded-full \
                                       bg-danger-soft px-2 py-0.5 text-micro font-medium \
                                       text-danger-soft-fg";

/// A control a screen offers but cannot act on right now.
pub(crate) const BUTTON_DISABLED: &str = "inline-flex items-center gap-1.5 rounded-md border \
                                          border-line px-2.5 py-1 text-small font-medium \
                                          text-faint";

/// A standing notice about the answer being shown.
pub(crate) const NOTICE: &str = "border border-warn-soft-fg/30 bg-warn-soft text-small \
                                 text-warn-soft-fg";

/// A link inside prose.
pub(crate) const LINK: &str = "text-accent underline decoration-from-font underline-offset-2";

/// The mark for a fact the server did not state.
///
/// An em dash, not a sentence. Eleven cells that each say "Not loaded from an
/// artifact" make absence the densest thing on a screen; the sentence belongs
/// in the mark's own title, where a reader who wants it can find it.
pub(crate) const ABSENT: &str = "text-faint";

/// The character that mark draws.
pub(crate) const ABSENT_MARK: &str = "\u{2014}";

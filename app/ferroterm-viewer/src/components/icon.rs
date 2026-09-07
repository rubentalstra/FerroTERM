//! The glyphs the screens carry, drawn inline as SVG.
//!
//! Every glyph here sits beside text that already says what it means, so the
//! `<svg>` is `aria-hidden` and adds nothing to what a screen reader announces
//! (<https://www.w3.org/TR/wai-aria-1.2/#aria-hidden>). Where a glyph replaces
//! words that used to be visible, the control keeps its accessible name from
//! `sr-only` text at the same place, so the name a screen reader reads is the
//! one it read before.
//!
//! The shapes are written here rather than pulled from an icon crate. The
//! whole set is twenty-two stroked outlines, and `docs/viewer.md` section 3
//! records what each answer measured.

use leptos::prelude::*;

/// The classes every glyph carries: the size, and the baseline it sits on.
const BASE: &str = "inline-block h-4 w-4 shrink-0 align-[-0.15em]";

/// One glyph, as the body of a 24-by-24 stroked outline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Glyph(&'static str);

/// Four tiles: the overview of everything this server serves.
pub(crate) const OVERVIEW: Glyph = Glyph(concat!(
    r#"<rect x="3" y="3" width="7" height="7" rx="1"/>"#,
    r#"<rect x="14" y="3" width="7" height="7" rx="1"/>"#,
    r#"<rect x="3" y="14" width="7" height="7" rx="1"/>"#,
    r#"<rect x="14" y="14" width="7" height="7" rx="1"/>"#,
));

/// A parent over two children: walking a hierarchy.
pub(crate) const BROWSE: Glyph = Glyph(concat!(
    r#"<rect x="9" y="2" width="6" height="5" rx="1"/>"#,
    r#"<rect x="2" y="17" width="6" height="5" rx="1"/>"#,
    r#"<rect x="16" y="17" width="6" height="5" rx="1"/>"#,
    r#"<path d="M12 7v8"/><path d="M5 17v-2h14v2"/>"#,
));

/// A list gaining a row: expanding a value set.
pub(crate) const EXPAND: Glyph = Glyph(concat!(
    r#"<path d="M3 6h18"/><path d="M3 12h9"/><path d="M3 18h9"/>"#,
    r#"<path d="M18 13v8"/><path d="M14 17h8"/>"#,
));

/// A bulleted list: the value sets a server holds.
pub(crate) const VALUE_SETS: Glyph = Glyph(concat!(
    r#"<path d="M8 6h13"/><path d="M8 12h13"/><path d="M8 18h13"/>"#,
    r#"<path d="M3 6h.01"/><path d="M3 12h.01"/><path d="M3 18h.01"/>"#,
));

/// Two arrows crossing: a code in one system reaching a code in another.
pub(crate) const CONCEPT_MAPS: Glyph = Glyph(concat!(
    r#"<path d="M4 8h13"/><path d="m14 5 3 3-3 3"/>"#,
    r#"<path d="M20 16H7"/><path d="m10 13-3 3 3 3"/>"#,
));

/// Sliders: what this reader has set for themselves.
pub(crate) const SETTINGS: Glyph = Glyph(concat!(
    r#"<path d="M4 6h10"/><path d="M18 6h2"/>"#,
    r#"<path d="M4 12h4"/><path d="M12 12h8"/>"#,
    r#"<path d="M4 18h10"/><path d="M18 18h2"/>"#,
    r#"<circle cx="16" cy="6" r="2"/><circle cx="10" cy="12" r="2"/>"#,
    r#"<circle cx="16" cy="18" r="2"/>"#,
));

/// One trunk splitting: the four FHIR versions of one server.
pub(crate) const VERSION: Glyph = Glyph(concat!(
    r#"<path d="M6 3v12"/><circle cx="6" cy="18" r="3"/>"#,
    r#"<circle cx="18" cy="6" r="3"/>"#,
    r#"<path d="M18 9v1a5 5 0 0 1-5 5H6"/>"#,
));

/// A tick in a circle: the server answered.
pub(crate) const SERVING: Glyph = Glyph(concat!(
    r#"<circle cx="12" cy="12" r="9"/>"#,
    r#"<path d="m8.5 12.5 2.5 2.5 4.5-5"/>"#,
));

/// A cross in a circle: the server did not answer.
pub(crate) const UNREACHABLE: Glyph = Glyph(concat!(
    r#"<circle cx="12" cy="12" r="9"/>"#,
    r#"<path d="m15 9-6 6"/><path d="m9 9 6 6"/>"#,
));

/// A warning triangle: the server refused this request.
pub(crate) const FAILURE: Glyph = Glyph(concat!(
    r#"<path d="M10.3 4.3 2.5 18a2 2 0 0 0 1.7 3h15.6a2 2 0 0 0 1.7-3L13.7 4.3a2 2 0 0 0-3.4 0Z"/>"#,
    r#"<path d="M12 9v4"/><path d="M12 17h.01"/>"#,
));

/// A tick inside a shield: checking a code against what a system holds.
pub(crate) const VALIDATE: Glyph = Glyph(concat!(
    r#"<path d="M12 3 4 6v6c0 4.5 3.2 7.9 8 9 4.8-1.1 8-4.5 8-9V6Z"/>"#,
    r#"<path d="m9 12 2 2 4-4"/>"#,
));

/// An `i` in a circle: something worth knowing about what is on screen.
pub(crate) const NOTICE: Glyph = Glyph(concat!(
    r#"<circle cx="12" cy="12" r="9"/>"#,
    r#"<path d="M12 16v-5"/><path d="M12 8h.01"/>"#,
));

/// A chevron meeting a bar: back to the start of the result.
pub(crate) const PAGE_FIRST: Glyph = Glyph(concat!(
    r#"<path d="m17 17-5-5 5-5"/>"#,
    r#"<path d="M8 6v12"/>"#
));

/// A chevron pointing back: the page before this one.
pub(crate) const PAGE_PREVIOUS: Glyph = Glyph(r#"<path d="m14 17-5-5 5-5"/>"#);

/// A chevron pointing on: the page after this one.
pub(crate) const PAGE_NEXT: Glyph = Glyph(r#"<path d="m10 7 5 5-5 5"/>"#);

/// A chevron meeting a bar: on to the end of the result.
pub(crate) const PAGE_LAST: Glyph = Glyph(concat!(
    r#"<path d="m7 17 5-5-5-5"/>"#,
    r#"<path d="M16 6v12"/>"#,
));

/// A magnifier: asking the server for what matches.
pub(crate) const SEARCH: Glyph = Glyph(concat!(
    r#"<circle cx="11" cy="11" r="7"/>"#,
    r#"<path d="m21 21-4.3-4.3"/>"#,
));

/// An arrow leaving a frame: this link hands the browser to the server.
pub(crate) const EXTERNAL: Glyph = Glyph(concat!(
    r#"<path d="M15 3h6v6"/><path d="M10 14 21 3"/>"#,
    r#"<path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>"#,
));

/// A sun: the light theme.
pub(crate) const LIGHT: Glyph = Glyph(concat!(
    r#"<circle cx="12" cy="12" r="4"/>"#,
    r#"<path d="M12 2v2"/><path d="M12 20v2"/>"#,
    r#"<path d="M2 12h2"/><path d="M20 12h2"/>"#,
    r#"<path d="m4.9 4.9 1.4 1.4"/><path d="m17.7 17.7 1.4 1.4"/>"#,
    r#"<path d="m6.3 17.7-1.4 1.4"/><path d="m19.1 4.9-1.4 1.4"/>"#,
));

/// A crescent: the dark theme.
pub(crate) const DARK: Glyph = Glyph(r#"<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8Z"/>"#);

/// A chevron pointing down: this concept's children are listed.
pub(crate) const TWIST_OPEN: Glyph = Glyph(r#"<path d="m6 9 6 6 6-6"/>"#);

/// A chevron pointing right: this concept has children that are not listed.
pub(crate) const TWIST_CLOSED: Glyph = Glyph(r#"<path d="m9 6 6 6-6 6"/>"#);

/// Draws one glyph, hidden from assistive technology.
///
/// The outline is stroked in `currentColor`, so a glyph takes the colour of
/// the text it sits in and needs no palette of its own. It carries no meaning
/// a reader cannot get from the words beside it, which is what makes hiding it
/// correct rather than lossy.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn Icon(
    /// The shape to draw.
    glyph: Glyph,
    /// Classes appended to the shared ones, for a glyph that needs its own
    /// size or spacing.
    #[prop(optional)]
    class: &'static str,
) -> impl IntoView {
    view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
            focusable="false"
            class=format!("{BASE} {class}")
            inner_html=glyph.0
        ></svg>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every glyph the screens draw, so a new one is checked by adding it here.
    const ALL: [Glyph; 22] = [
        OVERVIEW,
        BROWSE,
        EXPAND,
        VALUE_SETS,
        CONCEPT_MAPS,
        SETTINGS,
        VERSION,
        SERVING,
        UNREACHABLE,
        FAILURE,
        VALIDATE,
        NOTICE,
        PAGE_FIRST,
        PAGE_PREVIOUS,
        PAGE_NEXT,
        PAGE_LAST,
        SEARCH,
        EXTERNAL,
        LIGHT,
        DARK,
        TWIST_OPEN,
        TWIST_CLOSED,
    ];

    #[test]
    fn every_glyph_is_a_run_of_closed_elements() {
        for glyph in ALL {
            let body = glyph.0;
            assert!(body.starts_with('<'), "a glyph body is markup: {body}");
            assert!(body.ends_with("/>"), "a glyph body ends closed: {body}");
            assert_eq!(
                body.matches('<').count(),
                body.matches("/>").count(),
                "every element a glyph opens is self-closed: {body}"
            );
        }
    }

    #[test]
    fn every_glyph_draws_something_different() {
        for (index, glyph) in ALL.into_iter().enumerate() {
            let repeated = ALL.into_iter().skip(index + 1).any(|other| other == glyph);
            assert!(
                !repeated,
                "two screens would draw the same shape for different things: {}",
                glyph.0
            );
        }
    }
}

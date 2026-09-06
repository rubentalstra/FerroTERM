//! The preferences a reader sets, held in signals and written to storage.

use std::str::FromStr;

use leptos::prelude::Effect;
use leptos::prelude::Get;
use leptos::prelude::RwSignal;
use leptos::prelude::Set;
use leptos::prelude::With;

use crate::fhir::version::FhirVersion;
use crate::paging::MAX_COUNT;
use crate::storage;
use crate::theme::ThemeMode;

/// Where the theme choice is stored.
const THEME_KEY: &str = "ferroterm.viewer.theme";
/// Where the default FHIR version is stored.
const VERSION_KEY: &str = "ferroterm.viewer.fhir-version";
/// Where the display language is stored.
const LANGUAGE_KEY: &str = "ferroterm.viewer.display-language";
/// Where the page size is stored.
const PAGE_SIZE_KEY: &str = "ferroterm.viewer.page-size";

/// The preferences the viewer keeps, one signal each.
///
/// The values are per viewer and live in `localStorage` alone: the server is
/// asked nothing about a reader and told nothing about one.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Settings {
    /// The theme in force.
    pub(crate) theme: RwSignal<ThemeMode>,
    /// The FHIR version used when a link carries none.
    pub(crate) version: RwSignal<FhirVersion>,
    /// The BCP 47 tag sent as `displayLanguage`, empty for the server default.
    pub(crate) language: RwSignal<String>,
    /// How many rows a paged screen asks for.
    pub(crate) page_size: RwSignal<u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: RwSignal::new(ThemeMode::default()),
            version: RwSignal::new(FhirVersion::default()),
            language: RwSignal::new(String::new()),
            page_size: RwSignal::new(50),
        }
    }
}

impl Settings {
    /// Reads the stored preferences, falling back to the defaults.
    ///
    /// A stored value that no longer names anything the viewer ships is
    /// treated as absent, so an old browser profile cannot wedge the shell.
    pub(crate) fn load() -> Self {
        let settings = Self::default();
        settings.theme.set(
            storage::read(THEME_KEY)
                .as_deref()
                .and_then(ThemeMode::from_key)
                .unwrap_or_else(ThemeMode::preferred),
        );
        if let Some(version) = storage::read(VERSION_KEY)
            .as_deref()
            .and_then(|text| FhirVersion::from_str(text).ok())
        {
            settings.version.set(version);
        }
        if let Some(language) = storage::read(LANGUAGE_KEY) {
            settings.language.set(language);
        }
        if let Some(size) = storage::read(PAGE_SIZE_KEY).and_then(|text| parse_page_size(&text)) {
            settings.page_size.set(size);
        }
        settings
    }

    /// Writes every change back to storage.
    ///
    /// Storage is the outside world, so syncing to it is what an `Effect` is
    /// for
    /// (<https://github.com/leptos-rs/book/blob/main/src/reactivity/working_with_signals.md>).
    pub(crate) fn persist(self) {
        Effect::new(move |_| storage::write(THEME_KEY, self.theme.get().key()));
        Effect::new(move |_| storage::write(VERSION_KEY, self.version.get().segment()));
        Effect::new(move |_| self.language.with(|tag| storage::write(LANGUAGE_KEY, tag)));
        Effect::new(move |_| {
            storage::write(PAGE_SIZE_KEY, &self.page_size.get().to_string());
        });
    }
}

/// Reads a page size a reader typed, or `None` when it names no usable size.
///
/// Anything outside `1..=MAX_COUNT` is refused rather than clamped, so the
/// screen can say the value was not taken instead of silently using another.
pub(crate) fn parse_page_size(text: &str) -> Option<u32> {
    let size = text.trim().parse::<u32>().ok()?;
    (1..=MAX_COUNT).contains(&size).then_some(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_page_size_is_read() {
        assert_eq!(
            parse_page_size(" 25 "),
            Some(25),
            "surrounding space is trimmed"
        );
    }

    #[test]
    fn a_page_size_outside_the_bounds_is_refused_rather_than_clamped() {
        assert_eq!(parse_page_size("0"), None, "a zero-row page never advances");
        assert_eq!(
            parse_page_size("1001"),
            None,
            "the reader is told the value was not taken"
        );
    }

    #[test]
    fn text_that_is_not_a_number_names_no_page_size() {
        assert_eq!(parse_page_size("twenty"), None);
        assert_eq!(
            parse_page_size("-5"),
            None,
            "a negative count is not a size"
        );
        assert_eq!(parse_page_size(""), None);
    }

    #[test]
    fn the_bounds_themselves_are_accepted() {
        assert_eq!(parse_page_size("1"), Some(1));
        assert_eq!(parse_page_size("1000"), Some(MAX_COUNT));
    }
}

//! The runs a reader has made, kept in this browser and nowhere else.
//!
//! A run is already a URL: every runner puts its parameters in the address, so
//! remembering one is remembering a link. That is what makes this list cheap,
//! and it is why nothing here holds an answer: the address is re-run when the
//! reader returns to it, so a remembered run can never show a stale answer
//! beside a live one.
//!
//! The list lives in `localStorage`, like every other per-viewer value. The
//! server is neither asked about it nor told about it.

use serde::Deserialize;
use serde::Serialize;

use crate::storage;

/// Where the list is stored.
const KEY: &str = "ferroterm.viewer.runs";

/// How many runs are kept.
///
/// Enough to walk back through an afternoon's comparisons, short of a list a
/// reader has to read rather than glance at. The oldest falls off the end.
const CAPACITY: usize = 12;

/// One run a reader made.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Run {
    /// The runner it was made on, as a reader reads it.
    pub(crate) screen: String,
    /// What it was about: the code, the canonical, the phrase.
    pub(crate) subject: String,
    /// The address that reproduces it.
    pub(crate) address: String,
}

/// The runs this browser holds, newest first.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Runs(Vec<Run>);

impl Runs {
    /// Reads a stored list, or an empty one when nothing usable is stored.
    ///
    /// A value that no longer parses is treated as absent rather than as an
    /// error, because an old browser profile must never wedge a runner.
    pub(crate) fn read(stored: Option<&str>) -> Self {
        stored
            .and_then(|text| serde_json::from_str::<Self>(text).ok())
            .map(Self::capped)
            .unwrap_or_default()
    }

    /// This list with `run` at the front.
    ///
    /// A run already in the list moves to the front rather than appearing
    /// twice, so re-running one address does not fill the list with it.
    pub(crate) fn remembering(&self, run: &Run) -> Self {
        let mut kept = vec![run.clone()];
        kept.extend(
            self.0
                .iter()
                .filter(|held| held.address != run.address)
                .cloned(),
        );
        Self(kept).capped()
    }

    /// The runs, newest first.
    pub(crate) fn entries(&self) -> &[Run] {
        &self.0
    }

    /// Whether the list holds nothing.
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// This list, trimmed to what is kept.
    fn capped(mut self) -> Self {
        self.0.truncate(CAPACITY);
        self
    }
}

/// The list this browser holds.
pub(crate) fn stored() -> Runs {
    Runs::read(storage::read(KEY).as_deref())
}

/// Puts `run` at the front of the list this browser holds.
///
/// Storage is the outside world, so a caller writes here from an `Effect`
/// (<https://github.com/leptos-rs/book/blob/main/src/reactivity/working_with_signals.md>).
/// A browser that refuses to store simply keeps no list, which is a reader
/// with no history rather than a broken runner.
pub(crate) fn remember(run: &Run) -> Runs {
    let kept = stored().remembering(run);
    if let Ok(text) = serde_json::to_string(&kept) {
        storage::write(KEY, &text);
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One run, named by its subject so a case reads.
    fn run(subject: &str) -> Run {
        Run {
            screen: "Expand".to_owned(),
            subject: subject.to_owned(),
            address: format!("/ui/expand?fhir=r5&url={subject}"),
        }
    }

    #[test]
    fn nothing_stored_is_an_empty_list() {
        assert!(Runs::read(None).is_empty());
        assert!(
            Runs::read(Some("")).is_empty(),
            "an empty value is nothing stored"
        );
    }

    #[test]
    fn a_stored_value_that_no_longer_parses_is_treated_as_absent() {
        assert!(
            Runs::read(Some("{\"runs\":[]}")).is_empty(),
            "an old shape must never wedge a runner"
        );
        assert!(Runs::read(Some("not json at all")).is_empty());
    }

    #[test]
    fn the_newest_run_is_first() {
        let held = Runs::default()
            .remembering(&run("a"))
            .remembering(&run("b"));
        assert_eq!(
            held.entries()
                .iter()
                .map(|held| held.subject.as_str())
                .collect::<Vec<&str>>(),
            ["b", "a"],
            "a reader looks for what they just did"
        );
    }

    #[test]
    fn re_running_one_address_moves_it_rather_than_repeating_it() {
        let held = Runs::default()
            .remembering(&run("a"))
            .remembering(&run("b"))
            .remembering(&run("a"));
        assert_eq!(
            held.entries().len(),
            2,
            "the list holds one of each address"
        );
        assert_eq!(
            held.entries().first().map(|held| held.subject.as_str()),
            Some("a"),
            "the one just re-run is the one at the front"
        );
    }

    #[test]
    fn the_oldest_run_falls_off_the_end() {
        let held = (0..CAPACITY + 3).fold(Runs::default(), |held, index| {
            held.remembering(&run(&index.to_string()))
        });
        assert_eq!(held.entries().len(), CAPACITY);
        assert_eq!(
            held.entries().last().map(|held| held.subject.as_str()),
            Some("3"),
            "the three oldest are gone, and in the order they were made"
        );
    }

    #[test]
    fn a_stored_list_longer_than_the_cap_is_read_back_capped() {
        let long = Runs(
            (0..CAPACITY + 5)
                .map(|index| run(&index.to_string()))
                .collect(),
        );
        let text = serde_json::to_string(&long).expect("the list serializes");
        assert_eq!(
            Runs::read(Some(&text)).entries().len(),
            CAPACITY,
            "a list written by an older build cannot grow this one"
        );
    }

    #[test]
    fn a_run_round_trips_through_storage_text() {
        let held = Runs::default().remembering(&run("http://snomed.info/sct"));
        let text = serde_json::to_string(&held).expect("the list serializes");
        assert_eq!(
            Runs::read(Some(&text)),
            held,
            "a reload shows the reader what they had"
        );
    }
}

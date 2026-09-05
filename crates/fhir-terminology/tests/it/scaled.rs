//! The generated edition the benchmarks measure over: it opens, and it holds
//! what it says it holds.
//!
//! The benches assert latency; these assert that the edition behind them is a
//! real one, so a bench that answers fast is not answering over an empty
//! store.

use std::sync::Arc;

use concept_graph::subsumption::Outcome;
use fhir_terminology::compose::{Expander, Options};
use fhir_terminology::provider::CodeSystemProvider;
use fhir_terminology::registry::Registry;
use fhir_terminology::snomed::{SYSTEM, SnomedProvider};

use ferroterm_testkit::scaled;

/// Small enough to write in a test, large enough to have a real tree.
const CONCEPTS: u32 = 500;

fn provider() -> (tempfile::TempDir, SnomedProvider) {
    let dir = tempfile::tempdir().expect("tempdir");
    scaled::write(dir.path(), CONCEPTS).expect("writes the edition");
    let provider = SnomedProvider::open(dir.path(), "en").expect("opens");
    (dir, provider)
}

#[test]
fn every_concept_is_locatable_and_displays_its_preferred_term() {
    let (_dir, p) = provider();
    for ordinal in [0, 1, 8, 63, CONCEPTS - 1] {
        let code = scaled::code(ordinal);
        let found = p
            .locate(&code)
            .expect("reads")
            .unwrap_or_else(|| panic!("{code} is in the edition"));
        assert_eq!(
            p.display(found.concept, Some("en")).expect("reads"),
            Some(format!("Synthetic concept {ordinal}")),
            "the display is the preferred synonym, never the fully specified name"
        );
        assert_eq!(
            p.display(found.concept, Some("nl")).expect("reads"),
            Some(format!("Synthetisch concept {ordinal}")),
            "the Dutch synonym is the preferred term of its language"
        );
    }
}

#[test]
fn the_root_subsumes_every_other_concept() {
    let (_dir, p) = provider();
    let hierarchy = p.hierarchy().expect("snomed declares subsumption");
    let root = p
        .locate(&scaled::code(scaled::ROOT))
        .expect("reads")
        .expect("the root")
        .concept;
    for ordinal in [1, 9, 100, CONCEPTS - 1] {
        let other = p
            .locate(&scaled::code(ordinal))
            .expect("reads")
            .expect("in the edition")
            .concept;
        assert_eq!(
            hierarchy.subsumes(root, other),
            Outcome::Subsumes,
            "the root is above concept {ordinal}"
        );
    }
}

#[test]
fn the_implicit_descendants_value_set_holds_the_whole_edition() {
    let (_dir, p) = provider();
    let root = scaled::code(scaled::ROOT);
    let mut registry = Registry::new();
    registry.register(Arc::new(p)).expect("registers");
    let value_set = registry
        .implicit_value_set(&format!("{SYSTEM}?fhir_vs=isa/{root}"))
        .expect("snomed claims the URI")
        .expect("well formed");
    let expansion = Expander::new(&registry)
        .expand(
            &value_set,
            &Options {
                count: Some(10),
                offset: 100,
                ..Options::default()
            },
        )
        .expect("expands");
    assert_eq!(
        expansion.total,
        u64::from(CONCEPTS),
        "self-inclusive descendants of the root are the whole edition"
    );
    assert_eq!(expansion.items.len(), 10, "the page is the count asked for");
}

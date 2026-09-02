//! Checks over the licensed release a developer keeps under `data/`.
//!
//! Ignored by default: the content is licence-gated and never committed
//! (`.claude/rules/vendored-inputs.md`). Run with
//! `cargo nextest run -p ferroterm-rf2 --run-ignored all` when a release is present.

use std::path::PathBuf;

use ferroterm_rf2::component::{Concept, Rows};
use ferroterm_rf2::edition::Edition;
use ferroterm_rf2::file::{ContentType, Release, ReleaseType};
use ferroterm_rf2::refset::{Members, ModuleDependencyMember};

fn local_release() -> Option<PathBuf> {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/snomed");
    std::fs::read_dir(data)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.is_dir())
}

#[test]
#[ignore = "needs a licensed RF2 release under data/snomed/"]
fn the_local_edition_loads_and_identifies_itself() {
    let Some(root) = local_release() else {
        panic!("no release under data/snomed/");
    };
    let release = Release::open(&root, ReleaseType::Snapshot).expect("release opens");
    assert!(release.files().len() > 20);
    let concept_file = release
        .of_type(&ContentType::Concept)
        .next()
        .expect("concept file");
    let mut concepts = 0_u64;
    for concept in Rows::<_, Concept>::open(&concept_file.path).expect("header") {
        concept.expect("every concept row parses");
        concepts += 1;
    }
    assert!(concepts > 100_000, "{concepts} concepts");
    let dependency_file = release
        .refsets()
        .find(|f| f.name.summary == "ModuleDependency")
        .expect("module dependency refset");
    let ContentType::Refset(kinds) = &dependency_file.name.content_type else {
        panic!("refset content type");
    };
    let members: Vec<ModuleDependencyMember> = Members::open(&dependency_file.path, kinds)
        .expect("header")
        .map(|m| {
            ModuleDependencyMember::try_from(m.expect("member parses")).expect("dependency view")
        })
        .collect();
    let edition = Edition::identify(&members, release.date()).expect("edition identifies");
    assert!(edition.version_uri().starts_with("http://snomed.info/sct/"));
    assert_eq!(edition.effective_time, release.date());
}

// SPDX-License-Identifier: BUSL-1.1
//! Reads the release date of the version being built from `CHANGELOG.md`
//! (`## [<version>] - <date>`) into `FERROTERM_RELEASE_DATE`, for
//! `CapabilityStatement.software.releaseDate`. An unreleased version has no
//! date and the statement omits it.

use std::path::Path;

fn main() {
    let changelog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md");
    println!("cargo:rerun-if-changed={}", changelog.display());
    let version = env!("CARGO_PKG_VERSION");
    let heading = format!("## [{version}] - ");
    if let Ok(text) = std::fs::read_to_string(&changelog)
        && let Some(date) = text
            .lines()
            .find_map(|line| line.strip_prefix(heading.as_str()))
    {
        println!("cargo:rustc-env=FERROTERM_RELEASE_DATE={}", date.trim());
    }
}

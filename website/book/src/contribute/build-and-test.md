# Build and test

This page is a short pointer for contributors. The deeper design and the working
discipline live in the repository, and this book keeps to what you need to get
building.

<!-- toc -->

## Read these first

- [`docs/architecture.md`](https://github.com/rubentalstra/FerroTERM/blob/main/docs/architecture.md):
  the design authority, with citations to the terminology-server and
  graph-reachability literature.
- [`CONTRIBUTING.md`](https://github.com/rubentalstra/FerroTERM/blob/main/CONTRIBUTING.md):
  the contribution rules, branches, commit signing, and pull-request checklist.

## The local gates

Once the Cargo workspace exists, the local gates mirror CI exactly. Run them
before you open a pull request:

```console
$ cargo fmt --all --check
$ cargo clippy --workspace --all-targets --all-features -- -D warnings
$ cargo nextest run --workspace --locked
$ cargo test --doc --workspace --locked
$ RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --document-private-items
$ cargo deny check
```

Scope any command to a crate with `-p <crate>` while you iterate, then run the
full `--workspace` gates before you push. Every cargo invocation uses `--locked`,
and `Cargo.lock` is committed.

The workflow lanes that check shell and YAML (`actionlint`, `zizmor`,
`shellcheck`) run on every change, including before any Rust exists. If you edit a
shell script, keep it clean at `shellcheck --severity=style`.

## The two layers

- `crates/ferroterm-fhir` is generated from the vendored FHIR specs. Never hand-edit
  a `// @generated` file. See [The codegen model](codegen.md).
- The SNOMED engine and the server are hand-written, idiomatic Rust, with the
  FHIR and SNOMED specifications as the authority.

## No SNOMED CT content in the repository

Tests use shaped, synthetic content only. Never commit real SNOMED CT concepts
from a release, and never commit an RF2 file. See
[Loading a SNOMED CT edition](../operate/loading-snomed.md) for the licensing
rule.

## Correctness oracle

Terminology answers are checked against Snowstorm as the reference server over the
same edition, and the ECL evaluator is tested against the published ECL grammar as
its own layer before the value-set surface depends on it.

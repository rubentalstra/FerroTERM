<!-- SPDX-License-Identifier: Apache-2.0 -->

# Contributing to FerroTERM

FerroTERM is a pure-Rust FHIR terminology server for SNOMED CT, LOINC, and other
clinical code systems, SNOMED CT first. It is in early
design; the architecture is recorded, with citations, in
[`docs/architecture.md`](docs/architecture.md), and the working discipline is in
[`CLAUDE.md`](CLAUDE.md). Read both before making a change.

## The two layers

- `crates/ferroterm-fhir` is **generated** from the vendored machine-readable FHIR
  specs. Never hand-edit a file marked `// @generated`; change the generator
  (`tools/ferroterm-fhir-codegen`) and regenerate.
- The SNOMED engine (`ferroterm-rf2`, `ferroterm-graph`, `ferroterm-store`, `ferroterm-text`,
  `ferroterm-ecl`, `ferroterm-terminology`) and the server (`app/ferroterm-server`) are
  **hand-written**, modern idiomatic Rust; the FHIR and SNOMED specifications
  are the authority.

## Build and test

Once the Cargo workspace exists, the local gates mirror CI exactly:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --locked
cargo test --doc --workspace --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --document-private-items
cargo deny check
```

Run the same commands scoped to a crate (`-p <crate>`) while iterating; run the
full `--workspace` gates before opening a pull request. All cargo invocations
use `--locked`; `Cargo.lock` is committed.

The workflow lanes that also run on shell and YAML (`actionlint`, `zizmor`,
`shellcheck`) run on every change, including before any Rust exists. If you edit
a shell script, keep it clean at `shellcheck --severity=style`.

## Branches and commits

- Branch names use conventional types: `feat/<slug>`, `fix/<slug>`,
  `chore/<slug>`, `docs/<slug>`, `refactor/<slug>`, `perf/<slug>`, `test/<slug>`,
  `ci/<slug>`, `build/<slug>`. Never force-push `main`.
- Commit messages describe only the change. Do **not** add AI/assistant
  attribution, co-author trailers, or "generated with" lines anywhere.
- **Sign your commits.** Configure commit signing (GPG, SSH, or S/MIME) so every
  commit is verified.

## Pull requests

- Keep changes compiling and tested at every step; do not defer compilation.
- Never weaken, skip, or delete a test to make a build pass.
- Update the changelog for any user-visible change once `CHANGELOG.md` exists.
- Every workflow `uses:` is pinned to a full commit SHA with a trailing version
  comment; keep it that way. `permissions:` is `{}` at the workflow level with
  the minimum granted per job, and no untrusted context is interpolated into a
  `run:` block; pass it through `env:`.

## Security

Report vulnerabilities privately. See [`SECURITY.md`](SECURITY.md). Do not open
a public issue for a security problem.

## Licensing

By contributing you agree that your contributions are licensed under the
project's Apache License 2.0 (see [`LICENSE`](LICENSE)), as section 5 of that
licence provides.

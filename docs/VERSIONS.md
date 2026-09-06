# Pinned version matrix

The single source of truth for every version pin in FerroTERM. If this file and a
config file (`Cargo.toml`, `rust-toolchain.toml`, a vendored package's
`PROVENANCE.md`, `CITATION.cff`) disagree, that is drift: fix it, do not let
either silently win. `scripts/checks/versions.sh` enforces the cross-file
agreement it can, and skips (loudly) what does not exist yet in this
discovery-phase repo.

## Product and citation version

- The product version is the workspace `version` in the root `Cargo.toml`
  (inherited by the server and the tools), currently **0.1.0**.
- `CITATION.cff` `version` tracks it exactly. The version guard fails if they
  disagree once `Cargo.toml` exists.

## Crate line (crates.io)

- The `crates/*` members carry their own lockstep version in each member's
  `Cargo.toml` and in every internal requirement of the root
  `[workspace.dependencies]` table; `scripts/release/publish-crates.sh version`
  prints the current value. It moves with the crates'
  packaged content (`.claude/rules/crates-publishing.md`), never with the
  product version. `scripts/checks/crate-version-guard.sh` enforces the bump
  and the lockstep; `scripts/checks/versions.sh` checks the members' metadata
  (README, LICENSE, `publish = true`).

## Language and runtime

| Item | Pin |
|---|---|
| Rust toolchain | 1.98.0 (stable, pinned in `rust-toolchain.toml`) |
| Edition | 2024 |
| Cargo resolver | 3 |
| MSRV | 1.98 (`rust-version` in the root `Cargo.toml`, checked by `cargo hack check --rust-version`); the deliverable is a binary, so MSRV tracks the pinned stable |

## openEHR: n/a

FerroTERM is a FHIR/SNOMED project; it has no openEHR pins.

## FHIR

Machine-generated per version from the vendored, pinned HL7 FHIR packages
(`tools/fhir-codegen/vendor/`, once vendored, see
`.claude/rules/vendored-inputs.md`). Each vendored package carries a
`PROVENANCE.md`; the guard checks it against this table.

| Package | Pin | Notes |
|---|---|---|
| `hl7.fhir.r4b.core` | 4.3.0 | **first generation implemented** |
| `hl7.fhir.r5.core` | 5.0.0 | |
| `hl7.fhir.r4.core` | 4.0.1 | |
| `hl7.fhir.r6.core` | 6.0.0-ballot5 | ballot-tracking generation (not GA); published on packages2.fhir.org |
| `hl7.terminology` (THO) | 7.3.0 | terminology content moves here in R5/R6 |


## SNOMED CT / ECL

| Item | Pin |
|---|---|
| ECL | 2.2 (the tag of the official grammar repository; `ECL.g4` and the example corpus vendored under `crates/sct-ecl/vendor/` by `scripts/vendor/ecl-grammar.sh`) |
| SNOMED CT content | **not pinned in-repo**: licence-gated, bring-your-own RF2 (International edition); the loaded edition+version is a runtime/deployment fact (`.claude/rules/snomed-terminology.md`) |

## Rust dependency pins

The authoritative, fully-pinned third-party crate set lives in the root
`Cargo.toml` `[workspace.dependencies]`. This file
does not duplicate crate versions; on any discrepancy, the manifest wins. Add a
dependency to a crate with `dep.workspace = true`.

## Viewer toolchain

The viewer (`app/ferroterm-viewer`) compiles to WebAssembly and is built by
Trunk, not by `cargo` alone. Its crate versions live in the root
`Cargo.toml` like every other dependency; the pins below are the tools.

| Item | Pin |
|---|---|
| Compilation target | `wasm32-unknown-unknown` (`rustup target add wasm32-unknown-unknown`) |
| Trunk | 0.21.14, required by `app/ferroterm-viewer/Trunk.toml` `trunk-version` |
| Tailwind CSS standalone CLI | 4.3.3, pinned in `Trunk.toml` `[tools] tailwindcss`; Trunk downloads it, so there is no Node and no npm |
| `wasm-bindgen` CLI | taken from `Cargo.lock`, which pins the `wasm-bindgen` crate |
| `leptosfmt` | 0.1.33, the `view!` macro formatter |
| Headless browser | `selenium/standalone-chromium:4.48.0-20260905`, pinned by index digest in `scripts/ui-e2e.sh`; the image carries Chromium and the chromedriver built against it, so the journeys never depend on a runner image's own browser |

## GitHub Actions pins

Every `uses:` in `.github/workflows/**` is pinned to a full commit SHA with a
trailing `# vX.Y.Z` comment (`.claude/rules/ci-cd.md`); Dependabot bumps them.

## Conformance suite

| Item | Pin |
|---|---|
| HL7 terminology ecosystem test cases | `HL7/fhir-tx-ecosystem-ig` at `eaec771d82fba4eac596c14963546f39b4ecffe7` (test cases 1.9.3), `tests/` only. The requirements are cited by their unversioned page (<https://hl7.org/fhir/uv/tx-ecosystem/requirements.html>): the versioned 1.9.3 path does not resolve, and this table is where the version is pinned. |
| FHIR Validator (`txTests` runner) | 6.10.4, `validator_cli.jar` sha256 `1106b9d58f9e363e47bea7c4fc065841e5fc91fe9d062775c3bfdd212bd653cc` |

Both pins live in `scripts/checks/tx-ecosystem.sh`; the pass list is
`conformance/tx-ecosystem/passing.txt`.

# Pinned version matrix

The single source of truth for every version pin in FerroTERM. If this file and a
config file (`Cargo.toml`, `rust-toolchain.toml`, a vendored package's
`PROVENANCE.md`, `CITATION.cff`) disagree, that is drift: fix it, do not let
either silently win. `scripts/checks/versions.sh` enforces the cross-file
agreement it can, and skips (loudly) what does not exist yet in this
discovery-phase repo.

## Product and citation version

- The product version is the workspace `version` in the root `Cargo.toml`
  (inherited by the crates), currently **0.0.3** (pre-release).
- `CITATION.cff` `version` tracks it exactly. The version guard fails if they
  disagree once `Cargo.toml` exists.

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
(`tools/ferroterm-fhir-codegen/vendor/`, once vendored, see
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
| ECL | 2.2 (grammar from the official ANTLR `ECL.g4`) |
| SNOMED CT content | **not pinned in-repo**: licence-gated, bring-your-own RF2 (International edition); the loaded edition+version is a runtime/deployment fact (`.claude/rules/snomed-terminology.md`) |

## Rust dependency pins

The authoritative, fully-pinned third-party crate set lives in the root
`Cargo.toml` `[workspace.dependencies]`. This file
does not duplicate crate versions; on any discrepancy, the manifest wins. Add a
dependency to a crate with `dep.workspace = true`.

## GitHub Actions pins

Every `uses:` in `.github/workflows/**` is pinned to a full commit SHA with a
trailing `# vX.Y.Z` comment (`.claude/rules/ci-cd.md`); Dependabot bumps them.

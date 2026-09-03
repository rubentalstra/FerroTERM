---
name: crate-names-and-publishing
description: The library crates carry plain crates.io names since 2026-09-03 (fhir-types, rf2, concept-graph, sct-ecl, fhir-terminology, ...), a separate lockstep crate line (0.1.0) with a bump guard, and Trusted Publishing lanes; first publish done by hand in batches
metadata:
  type: project
---

On 2026-09-03 the owner renamed every `crates/*` member to what it is, for
crates.io and FerroBRIDGE (#164, PR #167): `fhir-types` (was ferroterm-fhir),
`rf2`, `concept-graph`, `concept-store`, `designation-index`, `sct-ecl`,
`fhir-terminology`, `loinc`, `classification`, `dhd-thesaurus`, `gstandaard`,
`icd11`, `rxnorm-rrf`; the generator is `tools/fhir-codegen`. The server
binary stays `ferroterm`; `ferroterm-build` and `ferroterm-testkit` stay
unpublished. `snomed`, `snomed-rf2`, `snomed-ecl` are another project's
active crates; `ecl`, `fhir`, `fhir-model`, `fhir-rs`, `rrf` are taken.

The crates have their own lockstep version line (0.1.0 first), separate from
the product version; `scripts/checks/crate-version-guard.sh` (CI job and push
hook) forces a bump when packaged content changes, `no-crate-bump` is the
label escape. Publishing: `scripts/release/publish-crates.sh` per crate in
dependency order; the `crates` leg of release.yml and `publish-crates.yml`
use Trusted Publishing in the `crates-io` environment (owner is the required
reviewer). The rule is `.claude/rules/crates-publishing.md`.

**Why:** the owner wants the crates reusable from crates.io, with FerroEHR's
publishing discipline (lockstep bump rule, OIDC lanes, per-crate resumable
upload).

**How to apply:** crates.io rate-limits NEW crate names (about five, then one
per ten minutes): publish a first release in batches and retry; the script
counts "already exists" as done. The Trusted Publisher entries per crate are
the owner's step (#168): repository `rubentalstra/FerroTERM`, workflows
`release.yml` and `publish-crates.yml`, environment `crates-io`. The
`::loinc::`, `::icd11::`, `::classification::` paths exist because same-named
modules live in `fhir-terminology`, `ferroterm-build`, and the testkit. See
[[multi-version-program]], [[release-cut-cadence]].

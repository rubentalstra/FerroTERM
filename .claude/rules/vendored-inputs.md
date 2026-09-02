---
paths: ["scripts/vendor/*.sh", "tools/ferroterm-fhir-codegen/vendor/**", "crates/**/tests/fixtures/**"]
---

# Vendored inputs (FHIR packages) and the SNOMED content rule

Two kinds of external material touch this repo, and they are handled
oppositely: the FHIR packages are vendored verbatim as codegen input; SNOMED
CT content is NEVER committed.

## FHIR packages: vendored verbatim, pinned, provenance-stamped

The pinned FHIR packages are the codegen input (`codegen.md`). Every one is:

- **Fetched by a committed `scripts/vendor/*.sh` script.** Never hand-download
  into the tree, never hand-edit a vendored file, never paste a package in
  from a chat transcript. To refresh or extend: change the script, re-run it,
  commit the result.
- **Vendored verbatim** under `tools/ferroterm-fhir-codegen/vendor/<package>/`,
  byte-for-byte as HL7 publishes it.
- **Stamped with a `PROVENANCE.md`** recording the source (the FHIR package
  registry, <https://www.hl7.org/fhir/packages.html>), the exact package
  version pin, the fetch date, and the upstream license (HL7 material under
  its own terms), with the upstream `LICENSE`/`package.json` alongside.

| input | script | destination |
|---|---|---|
| `hl7.fhir.r4.core` 4.0.1 | `scripts/vendor/fhir-packages.sh` | `tools/ferroterm-fhir-codegen/vendor/hl7.fhir.r4.core/` |
| `hl7.fhir.r4b.core` 4.3.0 | `scripts/vendor/fhir-packages.sh` | `tools/ferroterm-fhir-codegen/vendor/hl7.fhir.r4b.core/` |
| `hl7.fhir.r5.core` 5.0.0 | `scripts/vendor/fhir-packages.sh` | `tools/ferroterm-fhir-codegen/vendor/hl7.fhir.r5.core/` |
| `hl7.fhir.r6.core` 6.0.0-ballot | `scripts/vendor/fhir-packages.sh` | `tools/ferroterm-fhir-codegen/vendor/hl7.fhir.r6.core/` |
| `hl7.terminology` (THO) | `scripts/vendor/fhir-packages.sh` | `tools/ferroterm-fhir-codegen/vendor/hl7.terminology/` |

The `PreToolUse` guard blocks a hand-edit of any file carrying an
`// @generated` marker; the vendored packages are protected by discipline +
review (a hand-edit of a vendored package is a defect to revert). A vendored
input is not done until the drift check exercises it (`codegen.md`).

## SNOMED CT content is NEVER committed (licence-gated)

SNOMED CT is licensed by SNOMED International (free within member countries,
affiliate licence elsewhere). **The repository ships no RF2 content and no
derived edition data:** no concepts, no descriptions, no relationships, no
transitive-closure file, no built `redb`/`fst`/roaring artifacts derived from a
release. A deployment brings its own licensed RF2 release; `tools/ferroterm-build`
turns it into the served artifacts offline, outside version control.
A developer with a licence keeps a release under `data/` (the whole directory
is gitignored), for example `data/snomed/<release>/Snapshot/`; that is the
local input for `ferroterm-build` and the reference-server comparison.

- **Test fixtures use shaped, synthetic content only:** a small, hand-built
  hierarchy invented for the test (synthetic SCTIDs, invented terms) that
  exercises subsumption, ECL, expansion, and search. **Never** extract real
  SNOMED concepts, descriptions, or terms from a release into a fixture, even
  a handful: that is redistribution of licensed content. The reference-server
  comparison (`testing.md`) runs against a locally-provisioned licensed
  edition that stays out of the repo.
- **`.gitignore` refuses RF2 and built artifacts** so a licensed release
  dropped into a working tree cannot be committed by accident (the
  `Snapshot`/`Full`/`Delta` RF2 layouts, `*.rf2`, `sct2_*`/`der2_*` files, and
  the build output directory). If a genuinely new content shape appears,
  widen `.gitignore` in the same change; never commit content to "just get it
  working".

When in doubt about whether a byte is licensed SNOMED content: it is, and it
does not go in the repository.

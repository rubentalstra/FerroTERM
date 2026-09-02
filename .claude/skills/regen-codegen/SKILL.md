---
name: regen-codegen
description: Regenerate the generated ferroterm-fhir crate from the vendored FHIR packages and verify no drift. Use after changing the generator, an override, or a vendored FHIR package pin.
allowed-tools: Bash, Read, Grep
---

# Regenerate the FHIR layer

`crates/ferroterm-fhir` is generated from the vendored, pinned FHIR packages by
`tools/ferroterm-fhir-codegen`. Never hand-edit a `// @generated` file: change the
generator or its override map and regenerate here. Full discipline:
`.claude/rules/codegen.md`.

> This workflow is the intended shape; the generator lands in a later milestone
> (see `docs/architecture.md`). Until `tools/ferroterm-fhir-codegen` exists these
> steps describe what to run, not a live command.

## Steps

1. **Confirm the inputs are the pinned packages.** The vendored FHIR packages
   (`hl7.fhir.r4.core`, `hl7.fhir.r4b.core`, `hl7.fhir.r5.core`,
   `hl7.fhir.r6.core`, `hl7.terminology`) live under
   `tools/ferroterm-fhir-codegen/vendor/` with a `PROVENANCE.md` each. They are
   fetched only by `scripts/vendor/fhir-packages.sh`, never hand-edited
   (`.claude/rules/vendored-inputs.md`).

2. **Regenerate** the per-version modules:

   ```bash
   cargo run -p ferroterm-fhir-codegen -- emit
   ```

3. **Verify no drift:** regeneration must be byte-deterministic and the working
   tree clean afterward:

   ```bash
   git diff --exit-code crates/ferroterm-fhir
   ```

   A non-empty diff means the committed generated code was stale; commit the
   regenerated output. CI runs the same check as a drift gate.

4. **Gate the result:**

   ```bash
   cargo build -p ferroterm-fhir
   cargo clippy -p ferroterm-fhir --all-targets -- -D warnings
   cargo nextest run -p ferroterm-fhir
   ```

## Rules

- The generator emits the COMPLETE model from the vendored inputs; never trim
  output to quiet a diff or dodge a build error.
- A shape a consumer needs but the generated crate lacks is fixed in the
  generator/override, never with a hand-written shadow type.
- Every generated file starts with `// @generated` and is off-limits to hand
  edits.

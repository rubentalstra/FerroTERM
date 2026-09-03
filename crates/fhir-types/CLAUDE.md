# fhir-types

The generated FHIR layer. Every `.rs` file here is produced by
`tools/fhir-codegen` from the vendored FHIR packages and is off-limits
to hand edits; the only hand-maintained files are `Cargo.toml` and this
`CLAUDE.md`. Full discipline: `.claude/rules/codegen.md`.

- To change anything under `src/`, change the emitter or its inputs, then run
  `cargo run -p fhir-codegen -- emit` and commit the regenerated tree.
  `cargo run -p fhir-codegen -- emit --check` is the CI drift check and
  fails on any difference.
- The emission scope is the declared terminology root set (Bundle,
  CapabilityStatement, CodeSystem, ConceptMap, OperationOutcome, Parameters,
  TerminologyCapabilities, ValueSet, and the terminology OperationDefinitions)
  and the complete closure of every type those roots reference, per version,
  one module per version (`r4b` and `r5` today). Never trim inside that closure.
- `Resource`-typed elements (`Bundle.entry.resource`, `contained`) hold the
  `Resource` enum over the root set plus `UnknownResource`, which keeps any
  other resource's JSON body so a Bundle round-trips.
- `doctest = false` is deliberate: generated doc text is not a curated
  example set. The crate-level `#![allow]` list in the generated `lib.rs`
  names the pedantic lints the specification's own text and shapes trip.

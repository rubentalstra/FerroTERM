# notio-fhir

The generated FHIR layer. Every file marked `// @generated` is produced by
`tools/notio-fhir-codegen` from the vendored FHIR packages and is off-limits
to hand edits. Full discipline: `.claude/rules/codegen.md`.

- To change anything here, change the emitter or its override map, then run
  `cargo run -p notio-fhir-codegen -- emit` and commit the regenerated tree.
  The CI drift check regenerates and fails on any diff.
- The emission scope is the declared terminology root set (CodeSystem,
  ValueSet, ConceptMap, Parameters, OperationOutcome, CapabilityStatement,
  TerminologyCapabilities, Bundle, and the terminology OperationDefinitions)
  and the complete closure of every type those roots reference, per version.
  Never trim inside that closure.
- `doctest = false` is deliberate: generated doc text is not a curated
  example set.
- The only hand-written file is this `CLAUDE.md`. `src/lib.rs` is the
  generator's output root; before the first emit it is a placeholder carrying
  the tracker TODO.

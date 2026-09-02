# notio-fhir-codegen

The generator: vendored FHIR packages in, `crates/notio-fhir` out. Hand-written
tooling; the vendored `StructureDefinition` and `OperationDefinition`
resources are the authority for what it emits (`.claude/rules/codegen.md`).

- Inputs live under `vendor/<package>/`, fetched only by
  `scripts/vendor/fhir-packages.sh`, verbatim, with a `PROVENANCE.md` each
  (`.claude/rules/vendored-inputs.md`). Never hand-edit a vendored file.
- The emission scope is the declared terminology root set and the complete
  closure of the types it references, per version. A shape the consumer lacks
  is fixed here, never shadowed downstream.
- Output is byte-deterministic: iterate `BTreeMap` and sorted vectors, never
  a hash map (`.claude/rules/reliability.md`).
- `main.rs` is thin over `lib.rs` so the loader and emitter are unit-tested
  through the library.
- This crate is a tool, so it may write to stdout and stderr; every such site
  carries a scoped `#[expect]` with a reason.

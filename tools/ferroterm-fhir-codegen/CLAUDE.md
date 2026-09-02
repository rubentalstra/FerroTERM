# ferroterm-fhir-codegen

The generator: vendored FHIR packages in, `crates/ferroterm-fhir` out. Hand-written
tooling; the vendored `StructureDefinition` and `OperationDefinition`
resources are the authority for what it emits (`.claude/rules/codegen.md`).

- Inputs live under `vendor/<package>/`, fetched only by
  `scripts/vendor/fhir-packages.sh`, verbatim, with a `PROVENANCE.md` each
  (`.claude/rules/vendored-inputs.md`). Never hand-edit a vendored file.
- The pipeline is `package` (read), `snapshot` (resolve), `roots` (select),
  `closure` (the root-set closure), `lower` (the Rust model: structs per
  type and backbone element, enums per choice, boxed cycle edges), `render`
  (source text), `emit` (write or `--check`). The emission scope is the
  declared root set and the complete closure of the types it references, per
  version. A shape the consumer lacks is fixed here, never shadowed downstream.
- Output is byte-deterministic: iterate `BTreeMap` and sorted vectors, never
  a hash map (`.claude/rules/reliability.md`); `rustfmt` from the pinned
  toolchain formats the output so `cargo fmt --check` and the emitter agree.
- `cargo run -p ferroterm-fhir-codegen -- emit` regenerates; `-- emit --check`
  is the drift check CI runs.
- `main.rs` is thin over `lib.rs` so the loader and emitter are tested
  through the library.
- This crate is a tool, so it may write to stdout and stderr; every such site
  carries a scoped `#[expect]` with a reason.

## Naming

Names say what a thing is in its domain, never which language or pipeline
stage produced it. The FHIR side keeps FHIR's names verbatim
(`fhir::StructureDefinition`, `fhir::ElementDefinition`,
`snapshot::ResolvedStructure`); the generated side names the artefact
(`lower::VersionModule` is the generated module for one FHIR version,
`lower::TypeDef` one type definition, `lower::Cardinality` a cardinality). A
`Rust`, `Fhir`, or `Gen` prefix on a type name is a smell: if two views of one
thing collide, the module path disambiguates (`fhir::` versus `lower::`).

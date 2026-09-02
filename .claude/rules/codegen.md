---
paths: ["crates/notio-fhir/**", "tools/notio-fhir-codegen/**"]
---

# Code generation: the `notio-fhir` discipline

The FHIR layer is GENERATED, not hand-written. HL7 publishes the whole type
system and every operation as machine-readable `StructureDefinition` and
`OperationDefinition` resources, in versioned packages. `crates/notio-fhir` is
produced deterministically from those packages by `tools/notio-fhir-codegen`;
the engine consumes the generated types as its FHIR model.

## The pipeline

vendored, pinned FHIR packages (`tools/notio-fhir-codegen/vendor/`, verbatim,
provenance-stamped, `vendored-inputs.md`) → `notio-fhir-codegen` (loader +
emitter) → `crates/notio-fhir`, one module per version (R4 / R4B / R5 / R6).

- **Regenerate:** `cargo run -p notio-fhir-codegen -- emit` (and the
  operation-contract emit). A `codegen-drift` check re-runs the generator in
  CI and fails on any diff, so the generated tree is always in sync with the
  vendored packages + the current emitter.
- **Pinned packages** (the versions the modules mirror): `hl7.fhir.r4.core`
  4.0.1, `hl7.fhir.r4b.core` 4.3.0, `hl7.fhir.r5.core` 5.0.0,
  `hl7.fhir.r6.core` 6.0.0-ballot, plus `hl7.terminology`. Exact pins +
  provenance live in each package's `PROVENANCE.md`; the fetcher is
  `scripts/vendor/*.sh`.

## The hard rules

- **Never hand-edit a `// @generated` file.** Every generated file starts with
  a `// @generated … DO NOT EDIT` banner. To change output, edit the emitter
  (or its override map), then regenerate, never the output. A doc defect, a
  wrong field type, a missing variant in a generated file is a
  `notio-fhir-codegen` fix + regeneration.
- **The emission scope is a DECLARED root-set closure, and it is emitted
  COMPLETE, never trimmed inside that closure.** Notio is a terminology
  server, not a full FHIR server, so the generator does NOT emit all ~150
  resources of each core package. The declared root set is the terminology
  surface (`CodeSystem`, `ValueSet`, `ConceptMap`, `Parameters`,
  `OperationOutcome`, `CapabilityStatement`, `TerminologyCapabilities`,
  `Bundle`, plus the terminology `OperationDefinition`s), and the generator
  emits the COMPLETE transitive closure of every datatype and primitive those
  roots reference, per version, at its version-mirrored location. Within that
  closure, completeness is absolute: never narrow a schema merge, prune a
  referenced type, or suppress a "missing" generated file to quiet a diff or
  dodge a build error. That is HIDING code that should exist. Widening or
  narrowing the root SET is a deliberate, recorded decision (a new operation
  or resource the server serves), never an ad-hoc per-file omission. A
  generation-side defect discovered en route is FIXED in the generator in the
  same change, not worked around.
- **Fix the emitter, never the consumer.** When engine code hits a shape in
  `notio-fhir` that is wrong or insufficient versus the vendored package (a
  missing field, a type too narrow, a per-version parameter absent), the fix
  is a `notio-fhir-codegen` emitter/override change + regeneration, NEVER a
  shadow type, a duplicate model, an adapter/re-modeling layer, a placeholder
  value, or a "temporary" local FHIR representation in the consumer. A
  consumer-side workaround silently forks the FHIR model and defeats the whole
  design. If the emitter fix is large, register a tracker issue. The
  workaround is still forbidden; on discovering an existing workaround,
  register its removal.
- **Per-version correctness is by construction.** The generator emits per
  FHIR version, so the operation surface and the parameter set are correct per
  version from the `OperationDefinition`, never from a hand-written
  conditional that can drift. A version difference the packages express is a
  generated difference; if two versions genuinely coincide the emitter may
  share, but the decision is the emitter's, driven by the inputs.
- **The output is byte-deterministic.** The emitter iterates ordered
  structures (`BTreeMap`/sorted vecs), so a regeneration with unchanged inputs
  produces an identical tree, which is what makes the drift check meaningful
  (`reliability.md` §Determinism).

## Where the boundary sits

`notio-fhir` is the ONLY generated crate. Everything else (RF2 loading, the
materialized graph, the store/text indexes, ECL, the terminology engine, the
server) is hand-written idiomatic Rust of our own design (`rust-style.md`),
consuming the generated FHIR types directly. The prior art for the generator
itself is Helios `hfs` (MIT, Rust, machine-generates per-version FHIR
modules); it is a client, not a server, and is read-only reference.

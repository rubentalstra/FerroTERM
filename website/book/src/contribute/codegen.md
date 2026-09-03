# The codegen model

The FHIR layer of FerroTERM is generated, not hand-written. This page explains the
model at the level a contributor needs. The mechanics live in the generator crate
and its rules in the repository.

<!-- toc -->

## Why generate the FHIR layer

HL7 publishes the whole FHIR type system and every operation as machine-readable
`StructureDefinition` and `OperationDefinition` resources, in versioned packages.
FerroTERM vendors the packages and generates per-version Rust modules from
them. Each version's
operation surface is then correct by construction: a parameter that R5 adds
appears in the R5 module because the R5 package declares it.

## The pinned inputs

The generator reads vendored, pinned FHIR packages:

| Package | Version |
|---|---|
| `hl7.fhir.r4.core` | 4.0.1 |
| `hl7.fhir.r4b.core` | 4.3.0 |
| `hl7.fhir.r5.core` | 5.0.0 |
| `hl7.fhir.r6.core` | 6.0.0-ballot5 |
| `hl7.terminology` | the HL7 Terminology release pinned in `docs/VERSIONS.md` |

The packages are vendored verbatim under `tools/fhir-codegen/vendor/`, each
with a `PROVENANCE.md`, and fetched by a script. You never hand-edit a vendored
package. Change the fetcher and re-run it.

## The rules

- **Never hand-edit a `// @generated` file.** To change the output, change the
  generator (`tools/fhir-codegen`) or its override map, then regenerate.
- **The generator emits the complete model within its declared closure.** A
  terminology server touches a small root set of resources, so the generator's
  root set is the terminology surface (`CodeSystem`, `ValueSet`, `ConceptMap`,
  `Parameters`, `OperationOutcome`, `CapabilityStatement`,
  `TerminologyCapabilities`, `Bundle`, and the terminology operations), and it
  emits the complete transitive closure of the datatypes those roots reference. It
  never trims inside that closure to quiet a diff, and it never adds a
  hand-written shape outside it.
- **A drift check regenerates in CI and fails on any diff**, so the generated
  layer stays in step with the vendored inputs.

## Regenerate

```console
$ cargo run -p fhir-codegen -- emit
```

Then run the drift check. If consuming code needs a shape the generated crate
lacks, fix the emitter rather than shadowing it with a hand-written type.

The generator design follows the sibling project
[FerroEHR](https://github.com/rubentalstra/FerroEHR), which generates its openEHR
model from vendored machine-readable specs the same way.

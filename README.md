# Notio

A pure-Rust FHIR terminology server for SNOMED CT. Machine-generated FHIR
support across R4, R4B, R5, and R6. No JVM, no Elasticsearch.

> **Notio** is a codename. The project's official name is not set yet, so it
> carries a working name until then. Notio is Latin for "concept" — a SNOMED CT
> terminology service is, at heart, a service that knows what each concept means
> and how the concepts relate.

> Status: early design. This repository is being scoped before implementation.
> The README describes the intended architecture and the decisions already
> taken, not a finished server.

## Why this exists

SNOMED CT terminology servers today are almost all Java, and the reference
server (Snowstorm) runs on Elasticsearch. That stack is capable, and it is
heavy: a machine serving the full International edition wants 16 to 32 GB of
RAM, most of it Elasticsearch heap, plus a search cluster to operate.

The data does not need that. The International edition is about 360,000
concepts, 1.2 million descriptions, and 1.5 million relationships. SNOMED
International's own lightweight server, Snowstorm Lite, drops Elasticsearch for
a single Lucene index and runs the full International edition in about 500 MB.
Notio takes that further: a single Rust binary, a memory-mapped index built once
per edition, and a footprint small enough to run beside other services on a
modest box.

## What it is

- A read-oriented FHIR terminology server for SNOMED CT.
- The HL7 FHIR terminology API — `CodeSystem/$lookup`, `CodeSystem/$subsumes`,
  `CodeSystem/$validate-code`, `ValueSet/$expand`, `ValueSet/$validate-code`,
  and `ConceptMap/$translate` — served across **R4, R4B, R5, and R6** from one
  running server.
- Value-set expansion driven by SNOMED's Expression Constraint Language (ECL).
- A single static binary. No JVM, no separate search service, no external
  database.

## The architecture, in brief

Four decisions define the project. They are recorded in full, with citations, in
[`docs/architecture.md`](docs/architecture.md).

- **Search is memory-mapped `fst` + `roaring`, not a general search engine.**
  SNOMED search is per-word prefix matching, in any order, filtered by language
  reference set and active status, ranked by matched-term length — a
  set-intersection problem, not a relevance-scoring one. A finite-state
  transducer (`fst`) holds the term dictionary and answers prefix and fuzzy
  lookups; roaring bitmaps hold the postings and the precomputed refset and
  status filters, intersected in one pass. Both are pure Rust and memory-mapped,
  so the index is built once per edition and the server starts in milliseconds.
- **FHIR support is machine-generated from the official specs, never
  hand-written.** HL7 publishes the whole type system and every operation as
  machine-readable `StructureDefinition` and `OperationDefinition` resources, in
  versioned packages. A generator emits per-version Rust modules from those
  packages, so a new `$expand` parameter in R5 appears where the spec has it and
  is absent where it does not. This mirrors how the sibling project
  [FerroEHR](https://github.com/rubentalstra/FerroEHR) generates its openEHR
  model from the vendored machine-readable specs.
- **The concept store and the search index are separate, and both are
  memory-mapped.** The store answers "what is this code and what are its
  relationships"; the index answers "which codes match this text". Keeping them
  apart keeps each one simple and small — the shape Hermes uses in production.
- **The SNOMED semantics are hand-written and owned.** RF2 loading, the is-a
  subsumption graph, ECL evaluation, and `$expand` paging are the product. The
  generated FHIR layer gives standards-correct shapes and signatures by
  construction; the engine beneath it implements the meaning.

## The plan

The first milestone is a read-only core over the International edition: load an
RF2 snapshot, build the memory-mapped concept store and description index, and
answer `$lookup`, `$subsumes`, and `$validate-code`. `$expand` over a first
slice of ECL follows. Correctness is checked against Snowstorm as the reference
server, because a terminology server that is subtly wrong is worse than none.

ECL is the hard part and the main risk. A correct ECL evaluator (refinements,
attribute groups, cardinality, dotted attributes) is where the real work sits,
so it is built and tested as its own layer against the published ECL grammar
before the value-set surface depends on it.

## Licensing

Two separate things, and they must not be confused:

- **The software** in this repository is open source under the MIT license.
  Use it, embed it, run it.
- **SNOMED CT content** is licensed by SNOMED International and is not
  distributed here. Running Notio against the International edition requires a
  valid SNOMED CT licence, which is free within member countries (the
  Netherlands among them) and available under the affiliate licence elsewhere.
  You bring your own RF2 release and the server loads it.

## Relationship to FerroEHR

Notio is a standalone project. It grew out of
[FerroEHR](https://github.com/rubentalstra/FerroEHR), a pure-Rust openEHR
clinical data repository, which resolves archetype value-set bindings against an
external FHIR terminology server. openEHR places that resolver outside the CDR
by design, so Notio is a separate server reached over FHIR rather than a part of
the CDR, and it is useful to any FHIR client, not only to FerroEHR.

## Contributing

The project is at the design stage, so the most useful contributions right now
are review of the architecture in [`docs/architecture.md`](docs/architecture.md)
and of the ECL approach in particular. Issues and discussion are welcome.

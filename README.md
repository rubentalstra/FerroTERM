# FerroTERM

[![CI](https://github.com/rubentalstra/ferroterm/actions/workflows/ci.yml/badge.svg)](https://github.com/rubentalstra/ferroterm/actions/workflows/ci.yml)
[![CodeQL](https://github.com/rubentalstra/ferroterm/actions/workflows/codeql.yml/badge.svg)](https://github.com/rubentalstra/ferroterm/actions/workflows/codeql.yml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/rubentalstra/ferroterm/badge)](https://scorecard.dev/viewer/?uri=github.com/rubentalstra/ferroterm)
[![Quality Gate Status](https://sonarcloud.io/api/project_badges/measure?project=rubentalstra_ferroterm&metric=alert_status)](https://sonarcloud.io/summary/overall?id=rubentalstra_ferroterm)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A pure-Rust FHIR terminology server for SNOMED CT, LOINC, and other clinical
code systems, SNOMED CT first. Machine-generated FHIR support across R4, R4B,
R5, and R6. No JVM, no Elasticsearch.

> **FerroTERM** is the official name: Ferro for the Rust family it shares with
> [FerroEHR](https://github.com/rubentalstra/FerroEHR), TERM for terminology.
> The site is <https://ferroterm.eu>.

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
FerroTERM takes that further: a single Rust binary, a memory-mapped index built once
per edition, and a footprint small enough to run beside other services on a
modest box.

## What it is

- A read-oriented FHIR terminology server for clinical code systems. SNOMED CT
  is the first system it serves; LOINC and the systems recorded in
  [`docs/terminologies.md`](docs/terminologies.md) follow through the same
  engine seam, so no code system is a special case in the operations.
- The HL7 FHIR terminology API (`CodeSystem/$lookup`, `CodeSystem/$subsumes`,
  `CodeSystem/$validate-code`, `ValueSet/$expand`, `ValueSet/$validate-code`,
  and `ConceptMap/$translate`), served across **R4, R4B, R5, and R6** from one
  running server.
- Value-set expansion driven by SNOMED's Expression Constraint Language (ECL).
- A single static binary. No JVM, no separate search service, no external
  database.

## The architecture, in brief

The design is recorded in full, with citations, in
[`docs/architecture.md`](docs/architecture.md). The starting point is what SNOMED
CT actually is: a graph, and also a **formal ontology in the OWL 2 EL
profile**. That splits the system into two problems with different answers.

- **Offline: classification, once per release.** The is-a hierarchy a client
  queries is a reasoner output (the same result SNOMED ships in its inferred and
  transitive-closure files), computed once, not re-derived per query.
- **Online: serving, from precomputed structures.** SNOMED is stored as a graph,
  natively, but as **integer-keyed CSR adjacency arrays plus roaring bitmaps for
  the transitive closure**. Subsumption is then an O(1) bitmap test, a
  descendant set is a bitmap returned directly, and ECL refinement is bitmap set
  algebra. This is a graph *model* with an index-materialized *store*, not a
  graph database and not live traversal, both of which the evidence shows lose
  on exactly the deep-reachability queries ECL depends on. Text search is a
  separate `fst` index. Every production terminology server (Snowstorm,
  Ontoserver, Hermes) converges on this materialized-index shape; Hermes serves
  subsumption in tens of microseconds this way.
- **Persistence is `redb`:** a pure-Rust, memory-mapped, ACID embedded engine.
  Disk-backed and a real engine, so it is neither everything-in-RAM nor a
  hand-rolled format, while staying pure Rust and a single self-contained binary.
- **FHIR support is machine-generated from the official specs, never
  hand-written.** HL7 publishes the whole type system and every operation as
  machine-readable `StructureDefinition` and `OperationDefinition` resources, in
  versioned packages. A generator emits per-version Rust modules from those
  packages, so a new `$expand` parameter in R5 appears where the spec has it and
  is absent where it does not. This mirrors how the sibling project
  [FerroEHR](https://github.com/rubentalstra/FerroEHR) generates its openEHR
  model from the vendored machine-readable specs.
- **The engine is code-system-neutral, and the semantics are hand-written and
  owned.** The store, the hierarchy index, and the search index know concepts,
  designations, properties, and typed edges, never one system's identifiers.
  Each code system arrives through a loader and a provider (SNOMED CT: RF2
  loading, the materialized ontology, ECL evaluation; LOINC and the rest
  follow), and the FHIR operations talk to that seam only.
  The generated FHIR layer gives standards-correct shapes and signatures by
  construction; the engine beneath it implements the meaning.

## Why not reuse an existing crate

Rust already has pieces of this: the `snomed-rust` crates (`snomed-rf2`,
`snomed-ecl`), the `fhir-sdk` generated models (`fhir-model`), the Helios
`hfs`/`hts` server, the `rh-*` package-generated models, and `sct-rs`. Each
was read at source level on 2026-09-02, and none is a dependency here. The
reasons are specific, not a preference for writing everything ourselves:

- **`snomed-rf2` and `snomed-ecl`** parse RF2 by hard-coded header signatures
  rather than the refset descriptor, evaluate ECL by breadth-first search over
  in-memory hash maps and return hash sets, cite a grammar subset with
  deliberate leniencies the official grammar forbids, and have no persistence
  or index. FerroTERM compiles ECL to set algebra over a memory-mapped closure
  (`docs/architecture.md` decisions 1 and 4), so the evaluator cannot be
  shared. Their unimplemented-construct list is a useful checklist and is read
  as such.
- **`fhir-sdk`** generates STU3, R4B, and R5 only (no R4 4.0.1, no R6), types
  `decimal` as `f64` where FHIR requires precision to be preserved, accepts
  unknown JSON members silently, and generates from hand-edited definition
  files without provenance. The four-version, strict, provenance-stamped
  generation this project promises is not reachable from it. Its primitive
  extension pairing and its round-trip test harness over the official examples
  are borrowed as ideas.
- **Helios `hfs`/`hts`** is the closest competitor (four versions, JSON and
  XML, every terminology operation) and is read as a behavioural oracle beside
  Snowstorm. It uses `unsafe` in several crates, downloads specification
  content at build time, and answers subsumption with SQL recursive queries,
  each of which this project rules out.
- **`sct-rs`** is AGPL-3.0 and cannot be distributed under MIT.
- **`octofhir-ucum`** (Apache-2.0, tested against the UCUM suite) is the one
  candidate dependency, shortlisted for the UCUM provider when that lands.

The full evaluation and its sources are recorded on the tracker; the
architecture names each project in its prior-art section.

## The plan

**FHIR R4B is the first version implemented:** the current stable release of
the R4 line and a near-superset of R4, so an R4B-first build already serves the
R4-family terminology surface; R5, R4, and R6 (ballot) follow as further
generations.

The first milestone is a read-only core over the International edition: an
offline build that turns an RF2 release into the memory-mapped store, graph, and
text index, then a server answering `$lookup`, `$subsumes`, and `$validate-code`
on R4B. `$expand` over a first slice of ECL follows. Correctness is checked
against Snowstorm as the reference server, because a terminology server that is
subtly
wrong is worse than none.

ECL is the hard part and the main risk. A correct ECL evaluator (refinements,
attribute groups, cardinality, dotted attributes) is where the real work sits,
so it is built and tested as its own layer against the published ECL grammar
before the value-set surface depends on it.

## Licensing

Two separate things, and they must not be confused:

- **The software** in this repository is open source under the MIT license.
  Use it, embed it, run it.
- **SNOMED CT content** is licensed by SNOMED International and is not
  distributed here. Running FerroTERM against the International edition requires a
  valid SNOMED CT licence, which is free within member countries (the
  Netherlands among them) and available under the affiliate licence elsewhere.
  You bring your own RF2 release and the server loads it.

## Why it exists

FerroTERM is a standalone project with a simple motivation: run a clinical
terminology server on ordinary hardware. The reference servers are Java on
Elasticsearch and want 16 to 32 GB of RAM to serve the International edition,
more than a laptop or a small box has. FerroTERM is a pure-Rust server built to
serve the same edition in a few hundred megabytes, so it runs on a personal
computer.

It speaks the FHIR terminology API, so it is useful to any FHIR client.
[FerroEHR](https://github.com/rubentalstra/FerroEHR), a pure-Rust openEHR
clinical data repository by the same author, is one such client (openEHR
resolves archetype value-set bindings against an external FHIR terminology
server), but FerroTERM is independent of it and was not derived from it.

## Contributing

The project is at the design stage, so the most useful contributions right now
are review of the architecture in [`docs/architecture.md`](docs/architecture.md)
and of the ECL approach in particular. Issues and discussion are welcome.

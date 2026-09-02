---
name: architecture-decisions
description: Notio's locked, evidence-backed architecture (the OWL-EL two-problem split, the index-materialized graph store, machine-generated FHIR, and the pure-Rust stack)
metadata:
  type: project
---

Notio is a pure-Rust FHIR terminology server for SNOMED CT. The full design +
citations live in `docs/architecture.md`; these are the load-bearing decisions,
each owner-confirmed and adjudicated from cited research (two deep-research
streams, 2026-09-02).

- **SNOMED is an OWL 2 EL ontology, not merely a graph.** The system splits into
  OFFLINE classification (the inferred is-a hierarchy is a reasoner result: the
  International edition ships no transitive-closure file, so the default computes
  the closure from the shipped inferred Relationship file, and uses a distributed
  transitive-closure file as an opportunistic fast path where an edition includes
  one; an ELK-style reasoner is a later capability) and ONLINE serving from
  precomputed structures.
- **Graph MODEL, index-materialized STORE: never a graph database, never live
  traversal on the hot path.** Integer-keyed CSR adjacency (is-a + per-attribute)
  plus `roaring` transitive-closure bitmaps. Subsumption = O(1) bitmap test; ECL
  = bitmap set algebra; text search = a separate `fst` index. Evidence: every
  production server (Snowstorm, Ontoserver, Hermes) uses a materialized index;
  graph DBs lose on ECL's deep-reachability queries. This honors the owner's
  (correct) graph intuition while implementing it the fast way.
- **Persistence = `redb`** (pure-Rust, memory-mapped, ACID): the persistence
  FORMAT, not the query-time path. The hot closure is loaded RESIDENT at startup
  (ordinal-indexed `Vec<RoaringBitmap>` / zero-copy layout), never
  redb-get-and-deserialize per `$subsumes` (that would forfeit the µs goal). The
  columnar store stays on mmap for point reads. Closure ~100 to 300 MB resident.
- **Offline classification default = COMPUTE the transitive closure from the
  inferred Relationship file** (SNOMED ships NO transitive-closure file for the
  International edition, only a script); consume a shipped TC file only where an
  edition includes one. A reasoner (ELK-style) is a later capability.
- **FHIR is machine-generated** from the vendored official FHIR packages
  (StructureDefinition + OperationDefinition), a bespoke FerroEHR-style
  generator, per-version modules for **R4 + R4B + R5 + R6 (ballot)**, runtime
  version wrapper. Never hand-write FHIR types; never edit `// @generated`.
  **Start with R4B first** (owner call: current stable R4-family release,
  near-superset of R4, an R4B-first build serves the R4 surface); then R5, R4,
  R6. Codegen SCOPE = a declared terminology root-set closure (CodeSystem,
  ValueSet, ConceptMap, Parameters, OperationOutcome, CapabilityStatement,
  TerminologyCapabilities, Bundle + the terminology OperationDefinitions + their
  datatype/primitive closure), NOT all ~150 FHIR resources. ECL parser = `winnow`
  (not the pre-1.0 chumsky).
- **Pure Rust, single binary, no JVM / no Elasticsearch.** SNOMED CT content is
  licence-gated and NEVER committed (bring-your-own RF2); fixtures are
  shaped/synthetic.

Crate plan (see `docs/architecture.md`): `notio-fhir` (generated), `notio-rf2`,
`notio-graph`, `notio-store`, `notio-text`, `notio-ecl`, `notio-terminology`,
`app/notio-server`, `tools/notio-fhir-codegen`, `tools/notio-build`.

Build sequence: (1) FHIR codegen → `notio-fhir`, R4B first then R5/R4/R6;
(2) RF2 + offline build →
redb artifacts; (3) read-only serving core ($lookup/$subsumes/$validate-code);
(4) ECL + $expand + $translate; (5) hardening + packaging. Reference oracles:
Snowstorm + Hermes; the ECL ANTLR grammar. See [[owner-work-style]].

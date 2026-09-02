---
name: architecture-decisions
description: Notio's locked, evidence-backed architecture — the OWL-EL two-problem split, the index-materialized graph store, machine-generated FHIR, and the pure-Rust stack
metadata:
  type: project
---

Notio is a pure-Rust FHIR terminology server for SNOMED CT. The full design +
citations live in `docs/architecture.md`; these are the load-bearing decisions,
each owner-confirmed and adjudicated from cited research (two deep-research
streams, 2026-09-02).

- **SNOMED is an OWL 2 EL ontology, not merely a graph.** The system splits into
  OFFLINE classification (the inferred is-a hierarchy is a reasoner result — for
  v1 consume SNOMED's shipped inferred + transitive-closure files; an ELK-style
  reasoner is a later capability) and ONLINE serving from precomputed structures.
- **Graph MODEL, index-materialized STORE — never a graph database, never live
  traversal on the hot path.** Integer-keyed CSR adjacency (is-a + per-attribute)
  plus `roaring` transitive-closure bitmaps. Subsumption = O(1) bitmap test; ECL
  = bitmap set algebra; text search = a separate `fst` index. Evidence: every
  production server (Snowstorm, Ontoserver, Hermes) uses a materialized index;
  graph DBs lose on ECL's deep-reachability queries. This honors the owner's
  (correct) graph intuition while implementing it the fast way.
- **Persistence = `redb`** (pure-Rust, memory-mapped, ACID). Disk-backed and a
  real engine — not everything-in-RAM, not a hand-rolled format.
- **FHIR is machine-generated** from the vendored official FHIR packages
  (StructureDefinition + OperationDefinition), a bespoke FerroEHR-style
  generator, per-version modules for **R4 + R4B + R5 + R6 (ballot)**, runtime
  version wrapper. Never hand-write FHIR types; never edit `// @generated`.
- **Pure Rust, single binary, no JVM / no Elasticsearch.** SNOMED CT content is
  licence-gated and NEVER committed (bring-your-own RF2); fixtures are
  shaped/synthetic.

Crate plan (see `docs/architecture.md`): `notio-fhir` (generated), `notio-rf2`,
`notio-graph`, `notio-store`, `notio-text`, `notio-ecl`, `notio-terminology`,
`app/notio-server`, `tools/notio-fhir-codegen`, `tools/notio-build`.

Build sequence: (1) FHIR codegen → `notio-fhir`; (2) RF2 + offline build →
redb artifacts; (3) read-only serving core ($lookup/$subsumes/$validate-code);
(4) ECL + $expand + $translate; (5) hardening + packaging. Reference oracles:
Snowstorm + Hermes; the ECL ANTLR grammar. See [[owner-work-style]].

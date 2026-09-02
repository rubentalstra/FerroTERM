# Architecture

Notio (codename) is a pure-Rust FHIR terminology server for SNOMED CT. It serves
the HL7 FHIR terminology API across R4, R4B, R5, and R6 from one running server,
backed by a memory-mapped, precomputed SNOMED index, with no JVM, no
Elasticsearch, and no graph database.

This document is the design authority. It is grounded in the terminology-server
literature and the graph-reachability literature, cited inline, rather than in
convention.

## First principles: what SNOMED CT actually is

SNOMED CT is two things at once, and conflating them is the mistake most
architectures make.

1. **It is a formal ontology in the OWL 2 EL profile.** Concept definitions are
   description-logic axioms; the is-a hierarchy a client queries is not the
   *stated* hierarchy but the *inferred* one, produced by a description-logic
   reasoner (ELK, Snorocket, FaCT++) through consequence-based saturation
   (Kazakov, Krötzsch, Simančík, "The Incredible ELK", J. Automated Reasoning
   53(1), 2014, <https://link.springer.com/article/10.1007/s10817-013-9296-3>;
   SNOMED OWL Reference Set,
   <https://docs.snomed.org/snomed-ct-specifications/snomed-ct-owl-reference-set-specification>).
   ELK classifies all of SNOMED in about five seconds into roughly five million
   subsumptions — under 0.01% of all possible concept pairs, an extremely sparse
   relation.
2. **It is a polyhierarchical graph** — a concept has many parents; edges are
   typed (is-a, finding site, associated morphology, and the rest) (SNOMED
   Concept Model,
   <https://docs.snomed.org/snomed-ct-practical-guides/snomed-ct-starter-guide/6-snomed-ct-concept-model>).

The graph model is correct and universal. The consequence that most designs miss
is that the system therefore has **two separate problems with different
engineering answers**: an offline inference problem, and an online serving
problem. Everything below follows from keeping them apart.

## The two problems

### Offline: classification, once per release

The hierarchy Notio serves is a reasoner output, computed once per SNOMED
release, never at query time. Two valid ways to obtain it:

- **Consume SNOMED's shipped files** — the inferred relationship file plus the
  distributed transitive-closure file
  (<https://docs.snomed.org/snomed-ct-specifications/snomed-ct-release-file-specification/component-release-file-specification/4.2-file-format-specifications/4.2.5-transitive-closure-files>).
  This is the default for the first server: SNOMED already publishes the
  classification result, and its own guidance calls a precomputed transitive
  closure "one of the most efficient ways to test for subsumption"
  (<https://docs.snomed.org/snomed-ct-practical-guides/snomed-ct-data-analytics-guide/6-snomed-ct-analytic-techniques/6.2-subsumption>).
- **Run a reasoner** — an ELK-style consequence-based classifier over the OWL
  axiom refset. Needed only when the input is stated axioms or post-coordinated
  expressions that must be classified locally. This is a later capability, not a
  requirement to serve a released edition.

The build pipeline turns the release into the materialized serving structures
below. It runs offline (in a tool, `tools/notio-build`), so the running server
never classifies and never re-derives inference.

### Online: serving, from precomputed structures

Every FHIR terminology operation is answered from a precomputed structure, and
each operation has its own optimal shape. This table is the load-bearing design
decision:

| Operation | True query shape | Structure that answers it |
|---|---|---|
| `$subsumes` | deep ancestor test | roaring-bitmap membership — O(1) |
| `$expand` ECL `<<`/`<`/`>>`/`>` | reachability set | the precomputed descendant/ancestor bitmap, returned directly |
| `$expand` ECL refinement (`: attr = <<X`) | set intersection/union across typed edges | bitmap set-algebra over the per-attribute adjacency |
| `$validate-code` | point read + MRCM check | columnar concept/description store |
| `$lookup` (properties, designations) | point read + 1-hop neighborhood | columnar store + adjacency |
| `$lookup`/search by term, autocomplete | text match | a separate `fst` inverted/prefix index |
| `$translate` | map-refset lookup | key-value lookup |

The point the pure-graph view misses: "descend the hierarchy level by level" is
paid **once, offline**, and at runtime the descent collapses into a
set-membership test or a bitmap intersection. Graph traversal is the slow way to
answer what a terminology server is actually asked.

## The defining decisions

### 1. Graph model, index-materialized storage — not a graph database, not live traversal

SNOMED is stored as a graph, natively: integer-keyed (SCTID) **compressed
sparse-row (CSR) adjacency arrays** — is-a in both directions, plus one CSR per
attribute type for ECL refinement — and **roaring bitmaps** holding the
transitive closure (each concept's ancestor and descendant sets). CSR arrays are
graph-native and cache-friendly; they are not a pointer graph and not a database.

A general-purpose graph database is rejected on the evidence, not on taste:

- No production terminology server uses one. Ontoserver is Postgres + a Lucene
  index (Metke-Jimenez et al., "Ontoserver: a syndicated terminology server", J.
  Biomedical Semantics 9:24, 2018,
  <https://jbiomedsem.biomedcentral.com/articles/10.1186/s13326-018-0191-z>);
  Snowstorm is Elasticsearch (<https://github.com/IHTSDO/snowstorm>); Hermes is a
  memory-mapped store plus Lucene (<https://github.com/wardle/hermes>). All three
  chose materialized index structures.
- The graph-database speed wins in the literature benchmark graph traversal
  against naive relational JOINs, not against a precomputed index (Jeon 2025,
  <https://www.medrxiv.org/content/10.1101/2025.07.20.25322556v1>; Campbell et
  al. 2015, <https://www.sciencedirect.com/science/article/pii/S1532046415001847>),
  and a triple store lost to a relational store on query time even where it won
  on load time (Can et al., Entropy 19(1):30, 2017,
  <https://www.mdpi.com/1099-4300/19/1/30>).
- Graph databases degrade exactly where a terminology server is stressed —
  out-of-memory on large multi-hop neighborhoods, slow on high-degree subgraphs
  (<https://pmc.ncbi.nlm.nih.gov/articles/PMC7233100/>) — which is ECL
  descendant expansion of a high-level concept.

At SNOMED's size (~360k concepts, ~1.5M edges, static between releases) the full
transitive closure is the degenerate-optimal case of the whole reachability-index
family (2-hop, interval, GRAIL): when the closure fits in memory, it *is* the
best reachability index — O(1) test, no traversal fallback (Cohen et al., 2-hop,
SODA 2002, <https://dl.acm.org/doi/10.5555/545381.545503>; Agrawal et al.,
interval labeling, SIGMOD 1989, <https://dl.acm.org/doi/10.1145/67544.66950>).
Hermes demonstrates the payoff: ~13–69 µs subsumption and ~0.82 µs concept lookup
on a laptop, from a materialized structure rather than a graph walk.

Live adjacency traversal is kept for the queries it is genuinely best at — the
immediate parents/children and neighbourhood browsing of `$lookup` — and nothing
else routes through it.

### 2. FHIR support is machine-generated across every version

HL7 publishes the whole type system and every operation as machine-readable
`StructureDefinition` and `OperationDefinition` resources, in versioned packages
(`hl7.fhir.r4.core` 4.0.1, `hl7.fhir.r4b.core` 4.3.0, `hl7.fhir.r5.core` 5.0.0,
`hl7.fhir.r6.core` 6.0.0 ballot), plus `hl7.terminology`
(<https://www.hl7.org/fhir/packages.html>). Notio vendors and pins those
packages and generates per-version Rust modules from them
(`tools/notio-fhir-codegen`), so R5's extra `$expand` parameters
(`useSupplement`, `property`, `displayLanguage`) appear where the spec has them
and are absent where it does not
(<http://hl7.org/fhir/R5/valueset-operation-expand.html>). This mirrors how the
sibling project FerroEHR generates its openEHR model. Every generated file is
marked `// @generated`; a drift check regenerates in CI and fails on any diff;
the generator is never hand-edited. Versions are selected by a runtime wrapper,
so one server answers R4/R4B/R5/R6 callers at once. R6 is a ballot-tracking
generation (publication expected around late 2026).

### 3. Persistence is a pure-Rust, memory-mapped embedded engine

The built structures — the CSR adjacency, the roaring closure bitmaps, the
columnar concept/description store — are persisted in **`redb`**, a pure-Rust,
memory-mapped, ACID embedded key-value engine. This is deliberate: it is
disk-backed and a real storage engine, so it is neither everything-in-RAM nor a
hand-rolled file format, while staying pure Rust and a single self-contained
binary with no external service. The memory-mapped design gives millisecond
startup and an elastic resident set — the shape Hermes runs in production
(memory-mapped store), realized in pure Rust. The build tool writes these
artifacts once per edition; the server opens them read-only.

### 4. The SNOMED semantics are hand-written and owned

RF2 loading, the materialized ontology, ECL evaluation, and `$expand` paging are
the product, and no existing Rust project provides them. ECL is the hard part and
the main risk: every expression constraint returns a *set of codes* (SNOMED ECL
specification,
<https://docs.snomed.org/snomed-ct-specifications/snomed-ct-expression-constraint-language>),
so the evaluator compiles ECL to **set algebra** over the descendant bitmaps and
the attribute adjacency — bitmap AND/OR, not pointer-chasing. It is built and
tested as its own layer against the published ECL ANTLR grammar
(<https://github.com/IHTSDO/snomed-expression-constraint-language>) before the
value-set surface depends on it. Correctness is measured against Snowstorm as the
reference server.

A declarative alternative was considered and deferred: expressing subsumption and
ECL as Datalog over an embedded engine (`cozo`, pure-Rust, recursive queries
native). Datalog is a sound way to *compute and express* the closure, but for a
latency-critical path the materialized bitmap closure is the hot primitive; a
Datalog layer is a possible later addition for complex ad-hoc ECL, not the v1
foundation.

## Text search

Description search is a separate concern from the graph, kept in its own index.
SNOMED search is per-word prefix matching in any order, filtered by language
reference set and active status, ranked by matched-term length — not relevance
scoring (Snowstorm search docs,
<https://github.com/IHTSDO/snowstorm/blob/master/docs/search.md>). A finite-state
transducer (`fst`) holds the term dictionary for prefix and fuzzy lookup;
roaring bitmaps carry the refset and status filters, intersected in one pass.
Pure Rust, memory-mapped, built once per edition.

## Workspace layout

A single Cargo workspace. `notio-fhir` is generated; the rest is hand-written;
`notio-fhir-codegen` and `notio-build` are tooling.

| Crate | Role | Kind |
|---|---|---|
| `crates/notio-fhir` | Generated per-version FHIR types + terminology operation contracts (R4/R4B/R5/R6) | generated |
| `crates/notio-rf2` | SNOMED CT RF2 loader (inferred relationships, descriptions, refsets, transitive-closure file) + typed component model | hand-written |
| `crates/notio-graph` | The materialized ontology: CSR adjacency (is-a + per-attribute) and roaring transitive-closure bitmaps; subsumption + ECL set algebra | hand-written |
| `crates/notio-store` | The memory-mapped (`redb`) columnar concept/description store: point reads for `$lookup`/`$validate-code` | hand-written |
| `crates/notio-text` | The `fst` + roaring description search index (prefix, refset/status filter, term-length sort) | hand-written |
| `crates/notio-ecl` | Expression Constraint Language lexer, parser, and evaluator (compiles ECL to set algebra over `notio-graph`) | hand-written |
| `crates/notio-terminology` | The engine: the FHIR terminology operations over store + graph + text + ecl, dispatched per version | hand-written |
| `app/notio-server` | The `axum` HTTP server: FHIR endpoints, content negotiation, runtime version routing | hand-written |
| `tools/notio-fhir-codegen` | The generator: vendored FHIR packages → `notio-fhir` | tooling |
| `tools/notio-build` | The offline build: an RF2 release → the memory-mapped graph/store/text artifacts, once per edition | tooling |

Dependencies point one way (app/tools → crates); nothing depends upward into the
server.

## Verification

- **Snowstorm and Hermes as reference** for terminology answers: `$lookup`,
  `$subsumes`, `$validate-code`, `$expand` results are checked against the
  reference servers over the same edition.
- **ECL against its published grammar**: the evaluator is tested as its own layer
  before the value-set surface leans on it.
- **Classification parity**: when a reasoner is added, its inferred hierarchy is
  checked against SNOMED's shipped inferred/transitive-closure files.
- **Generated-layer drift check**: the FHIR generator is re-run in CI and fails
  on any diff.

## Licensing

The software is MIT. SNOMED CT content is licensed separately by SNOMED
International and is never distributed here — a deployment brings its own RF2
release under a valid licence (free within member countries, affiliate licence
elsewhere). The vendored FHIR packages are HL7 material under their own terms,
vendored verbatim with provenance as codegen input.

## Prior art

- **Snowstorm** (SNOMED International, Java + Elasticsearch) — the reference
  server and correctness oracle.
- **Ontoserver** (CSIRO, Postgres + Lucene) — a production FHIR terminology
  server; index-materialized, not a graph database.
- **Hermes** (Mark Wardle, Clojure — memory-mapped store + Lucene) — the
  memory-mapped, materialized-structure design, at microsecond latencies.
- **ELK / Snorocket** — the OWL 2 EL reasoners that classify SNOMED offline.
- **Helios `hfs`** (MIT, Rust) — machine-generates per-version FHIR modules; a
  client, not a server; prior art for the generator.

No existing Rust project is a complete, standards-generated, multi-version FHIR
terminology server. Notio fills that gap, built the way the evidence supports: a
graph model, an offline classification pass, and a memory-mapped
index-materialized store — not a graph database, and not live traversal.

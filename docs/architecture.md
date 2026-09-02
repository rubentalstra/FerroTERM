# Architecture

Notio (codename) is a pure-Rust FHIR terminology server for SNOMED CT. It serves
the HL7 FHIR terminology API across R4, R4B, R5, and R6 from one running server,
backed by a memory-mapped SNOMED index, with no JVM and no Elasticsearch.

Two layers, the same split the sibling project FerroEHR uses:

1. **The FHIR foundation — generated from the official machine-readable specs.**
   The FHIR type system and the terminology operation surface are emitted by our
   own generator from the vendored FHIR packages. Standards-correct by
   construction, and multi-version because the generator reads each version's own
   definitions.
2. **The SNOMED engine — hand-written, and the product.** RF2 loading, the
   concept store, the subsumption graph, the description search index, and ECL
   evaluation are our own design. The generated FHIR layer gives correct shapes
   and signatures; the engine gives the meaning.

## The four defining decisions

### 1. Search is memory-mapped `fst` + `roaring`, not a general search engine

SNOMED description search is a specific, narrow problem. Snowstorm — the
reference server — matches on **per-word prefix, in any order, filtered by
language reference set and active status, and sorts by matched-term length**,
with no fuzzy matching by default
(<https://github.com/IHTSDO/snowstorm/blob/master/docs/search.md>). That is a
set-intersection problem, not a relevance-scoring one, so a general-purpose
engine's BM25 machinery is weight we would carry and not use.

Notio's index is two pure-Rust pieces, both memory-mapped:

- A **finite-state transducer** (the `fst` crate) holds the term dictionary and
  answers per-word prefix, range, regex, and Levenshtein-fuzzy lookups, returning
  term ids. FSTs compress on shared prefixes and suffixes and are designed to be
  mmap'd (<https://crates.io/crates/fst>, <https://burntsushi.net/transducers/>).
- **Roaring bitmaps** (the `roaring` crate) hold the postings — term id to the
  set of descriptions — and the precomputed filters (each language refset, the
  active-status set). A query is a prefix lookup followed by bitmap
  intersections, then a sort by matched-term length
  (<https://docs.rs/roaring/>).

The index is **built once per SNOMED edition** and shipped as an artifact, so the
server memory-maps it and starts in milliseconds rather than re-tokenizing 1.2
million descriptions on every boot. SNOMED International's own Snowstorm Lite
proves the memory target with a single Lucene index — the full International
edition in about 500 MB (<https://github.com/IHTSDO/snowstorm-lite>); the
`fst` + `roaring` form is lighter still and pure Rust.

Ranking beyond matched-term length is deliberately out of the core. Semantic or
vector search, if ever wanted, is an additive layer, not part of the first
server.

### 2. FHIR support is machine-generated across every version

HL7 publishes the entire FHIR type system and every operation as machine-readable
`StructureDefinition` and `OperationDefinition` resources, distributed as
versioned packages — `hl7.fhir.r4.core` (4.0.1), `hl7.fhir.r4b.core` (4.3.0),
`hl7.fhir.r5.core` (5.0.0), and `hl7.fhir.r6.core` (6.0.0 ballot)
(<https://www.hl7.org/fhir/packages.html>). A `StructureDefinition` gives each
field's path, cardinality, type, and value-set binding; an `OperationDefinition`
gives an operation's parameters, their in/out use, cardinality, and types
(<https://www.hl7.org/fhir/structuredefinition.profile.json.html>,
<http://hl7.org/fhir/operationdefinition.html>).

Notio vendors those packages, pins them, and generates per-version Rust modules
from them with its own generator (`tools/notio-fhir-codegen`). This is the reason
multi-version support is tractable: the operation surface grows between versions —
R5 adds `displayLanguage`, `useSupplement`, and `property` to `$expand`, and
`useSupplement` to `$lookup`, none present in R4
(<http://hl7.org/fhir/R5/valueset-operation-expand.html>) — and a generator reads
each of those from the version's own definitions rather than a human tracking the
deltas. Every generated file is marked `// @generated` and a drift-check job
regenerates and fails on any diff. This is the FerroEHR discipline: never
hand-edit a generated file; change the emitter and regenerate.

Versions are selected at runtime through a wrapper, so one server instance can
answer R4, R4B, R5, and R6 callers at once — the expected shape for a shared
terminology service.

R6 is in ballot, not released (publication expected around late 2026,
<https://confluence.hl7.org/spaces/FMG/pages/256509111/Release+Plan+for+R6>), so
it is carried as a ballot-tracking generation, refreshed as the ballot moves —
the same way FerroEHR carries development-generation spec pins.

### 3. The concept store and the search index are separate, both memory-mapped

Two questions, two structures. The **store** answers "what is this code, and what
are its relationships" — the typed SNOMED components loaded from RF2, plus the
is-a graph as roaring ancestor and descendant bitmaps so subsumption is a bitmap
test. The **index** answers "which codes match this text" — the `fst` + `roaring`
description index above. Keeping them apart keeps each simple and small, and both
are memory-mapped so the resident set is elastic under the OS page cache. This is
the shape Hermes runs in production: a memory-mapped component store beside a
separate search index (<https://github.com/wardle/hermes/blob/main/doc/development.md>).

### 4. The SNOMED semantics are hand-written and owned

RF2 loading, the subsumption graph, ECL evaluation, and `$expand` paging are the
product, and no existing Rust project provides them. ECL is the hard part and the
main risk: a correct evaluator (refinements, attribute groups, cardinality,
dotted attributes) is built and tested as its own layer against the published ECL
grammar before the value-set surface depends on it. Correctness is measured
against Snowstorm as the reference server; a terminology server that is subtly
wrong is worse than none.

## The FHIR terminology API surface

Served across R4, R4B, R5, and R6, generated per version:

- **CodeSystem**: `$lookup`, `$validate-code`, `$subsumes`.
- **ValueSet**: `$expand`, `$validate-code`.
- **ConceptMap**: `$translate`.

The resources (`CodeSystem`, `ValueSet`, `ConceptMap`, `Parameters`,
`TerminologyCapabilities`) are generated types. The operations are generated
request and response signatures, implemented once against the engine and
dispatched per version. Terminology content that FHIR ships in the
`hl7.terminology` (THO) package is vendored as a first-class pinned input, since
R5 and R6 move content there (<https://build.fhir.org/terminologies.html>).

## Workspace layout

A single Cargo workspace. `notio-fhir` is generated; the rest is hand-written;
`notio-fhir-codegen` is tooling. The split mirrors FerroEHR.

| Crate | Role | Kind |
|---|---|---|
| `crates/notio-fhir` | Generated per-version FHIR types + terminology operation contracts (R4/R4B/R5/R6) | generated |
| `crates/notio-rf2` | SNOMED CT RF2 loader + typed component model | hand-written |
| `crates/notio-store` | Memory-mapped concept/relationship store + roaring subsumption bitmaps | hand-written |
| `crates/notio-index` | Memory-mapped `fst` + `roaring` description search index | hand-written |
| `crates/notio-ecl` | Expression Constraint Language lexer, parser, and evaluator | hand-written |
| `crates/notio-terminology` | The engine: implements the FHIR terminology operations over store + index + ecl, per version | hand-written |
| `app/notio-server` | The `axum` HTTP server: FHIR endpoints, content negotiation, version routing | hand-written |
| `tools/notio-fhir-codegen` | The generator: vendored FHIR packages to `notio-fhir` | tooling |

Dependencies point one way: `notio-server` and `notio-terminology` consume the
generated `notio-fhir` types and the hand-written engine crates; nothing depends
upward into the server.

## Verification

- **Snowstorm as the reference** for terminology answers: `$lookup`, `$subsumes`,
  `$validate-code`, and `$expand` results are checked against Snowstorm over the
  same edition.
- **ECL against its published grammar**: the evaluator is tested as its own layer
  before the value-set surface leans on it.
- **Generated-layer drift check**: the FHIR generator is re-run in CI and fails
  on any diff, so the vendored specs and the emitted code never drift.
- **FHIR conformance**: the served operations are validated against the FHIR
  terminology service expectations for each version.

## Licensing

The software is MIT. SNOMED CT content is licensed separately by SNOMED
International and is never distributed here — a deployment brings its own RF2
release under a valid licence (free within member countries, affiliate licence
elsewhere). The vendored FHIR packages are HL7 material under their own terms and
are vendored verbatim with provenance, as codegen input.

## Prior art

- **Snowstorm** (SNOMED International, Java + Elasticsearch) — the reference
  server and the correctness oracle.
- **Snowstorm Lite** (SNOMED International, Java + one Lucene index, ~500 MB) —
  proves the memory target and the single-index, no-external-service shape.
- **Hermes** (Mark Wardle, Clojure — memory-mapped LMDB store + Lucene index) —
  the separate-store-and-index pattern.
- **Helios `hfs`** (MIT, Rust) — machine-generates per-version FHIR modules
  (R4/R4B/R5/R6) from StructureDefinitions; a terminology client, not a server;
  prior art for the generator.
- **fhir-sdk**, **fhirbolt** (Rust) — FHIR type generation and (de)serialization
  prior art.

No existing Rust project is a complete, standards-generated, multi-version FHIR
terminology server. Notio fills that gap.

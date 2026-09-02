# CLAUDE.md

**Notio (codename)** is a pure-Rust FHIR terminology server for SNOMED CT. It
serves the HL7 FHIR terminology API across R4, R4B, R5, and R6 from one running
server, backed by a memory-mapped SNOMED index, with no JVM and no
Elasticsearch. The name is a working codename (Latin "concept"); the official
name is not set yet.

The full design, with citations, is in **`docs/architecture.md`** — read it
first. This file is the working discipline.

## The two layers (the FerroEHR split, applied to FHIR + SNOMED)

- **`notio-fhir` is GENERATED** from the vendored, machine-readable FHIR specs
  (`StructureDefinition` + `OperationDefinition`, from the pinned
  `hl7.fhir.*.core` packages). Treat every file marked `// @generated` as
  off-limits: to change output, change the generator
  (`tools/notio-fhir-codegen`) and regenerate — never hand-edit generated code.
  The generator emits per-version modules (R4/R4B/R5/R6) so the operation
  surface is correct per version by construction.
- **The SNOMED engine is HAND-WRITTEN** and is the product: RF2 loading, the
  memory-mapped concept store and subsumption graph, the `fst` + `roaring`
  description index, ECL evaluation, and the FHIR terminology operations over
  them. Modern idiomatic Rust of our own design; the FHIR and SNOMED
  specifications are the authority.

## Repo map (a single Cargo workspace)

`crates/*` = libraries, `app/*` = the server binary, `tools/*` = dev/codegen
tooling not shipped in the server. Every crate carries its own `CLAUDE.md` with
crate-local discipline.

- `crates/notio-fhir` — generated per-version FHIR types + terminology operation
  contracts (`// @generated`).
- `crates/notio-rf2` — SNOMED CT RF2 loader (inferred relationships,
  descriptions, refsets, transitive-closure file) + typed component model.
- `crates/notio-graph` — the materialized ontology: integer-keyed CSR adjacency
  (is-a + per-attribute) and roaring transitive-closure bitmaps; subsumption and
  ECL set algebra.
- `crates/notio-store` — the memory-mapped (`redb`) columnar concept/description
  store: point reads for `$lookup`/`$validate-code`.
- `crates/notio-text` — the `fst` + `roaring` description search index (per-word
  prefix, refset/status filters, matched-term-length sort).
- `crates/notio-ecl` — Expression Constraint Language lexer, parser, evaluator
  (compiles ECL to set algebra over `notio-graph`).
- `crates/notio-terminology` — the engine: `$lookup`/`$validate-code`/`$expand`/
  `$subsumes`/`$translate` over store + graph + text + ecl, dispatched per FHIR
  version.
- `app/notio-server` — the `axum` HTTP server (FHIR endpoints, content
  negotiation, runtime version routing).
- `tools/notio-fhir-codegen` — the generator: vendored FHIR packages →
  `notio-fhir`.
- `tools/notio-build` — the offline build: an RF2 release → the memory-mapped
  graph/store/text artifacts, once per edition.

Dependencies point one way (app/tools → crates); nothing depends upward into the
server. SNOMED is a graph MODEL served from an index-materialized store (CSR
adjacency + roaring closure), never a graph database and never live traversal on
the hot path — see `docs/architecture.md` for the evidence.

## Code generation — read this first

The FHIR layer is generated, not hand-written. HL7 publishes the whole type
system and every operation as machine-readable resources in versioned packages
(`hl7.fhir.r4.core` 4.0.1, `hl7.fhir.r4b.core` 4.3.0, `hl7.fhir.r5.core` 5.0.0,
`hl7.fhir.r6.core` 6.0.0-ballot), plus `hl7.terminology` (THO). We vendor and
pin those packages under `tools/notio-fhir-codegen/vendor/` with a
`PROVENANCE.md` per package, and generate `crates/notio-fhir` from them.
**R4B is the first generation implemented** (current stable R4-family release, a
near-superset of R4); R5, R4, and R6 follow.

- **Regenerate:** `cargo run -p notio-fhir-codegen -- emit` (types) and the
  operation-contract emit; a `codegen-drift` check regenerates in CI and fails
  on any diff.
- **Never hand-edit a `// @generated` file.** Change the emitter or its override
  map, then regenerate.
- **The generator emits the complete model from the vendored inputs** — never
  trim or scope-down output to quiet a diff or dodge a build error. If consuming
  code needs a shape the generated crate lacks, fix the emitter, never shadow it
  with a hand-written type.

## Tech stack (pinned in the manifests)

Rust stable, edition 2024, resolver 3, pinned in `rust-toolchain.toml`. No
`unsafe` (`unsafe_code = "forbid"`). The authoritative dependency set is the root
`Cargo.toml [workspace.dependencies]`; add to a crate with `dep.workspace =
true`. The menu:

- **Ontology store/index:** `redb` (pure-Rust, memory-mapped, ACID embedded
  engine — the persistence substrate), `roaring` (transitive-closure bitmaps),
  `fst` (description term dictionary), `petgraph` (in-memory CSR/graph algorithms
  for the offline build). Pure Rust, memory-mapped, disk-backed. Not
  Elasticsearch, not a graph database, not a C storage/search library.
- **HTTP/async:** `axum`, `tower`, `tower-http`, `hyper`, `tokio`.
- **Parsing (ECL):** `logos` (lexer) + `winnow` (parser — preferred over
  `chumsky`, which is still pre-1.0 with churn; the ECL grammar must stay
  faithful to the official ANTLR `.g4`),
  diagnostics via `miette`/`ariadne`.
- **RF2:** `csv` (RF2 is tab-delimited), streamed.
- **Serde/formats:** `serde`, `serde_json`; FHIR JSON (and XML where a version
  needs it) through the generated codec.
- **Errors:** `thiserror` in libraries, `anyhow` only in the binary. No
  `unwrap`/`expect` outside tests.
- **IDs/time:** `jiff` for FHIR dates/times.
- **Observability:** `tracing`, `tracing-subscriber`.
- **Testing:** `cargo-nextest`, `insta` (snapshots), `proptest`, `reqwest` +
  `wiremock` (the Snowstorm oracle comparison and HTTP mocking).

Verify every crate version against crates.io/docs.rs at the moment it is added;
never pin from memory.

## Build and test

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items
```

Regeneration + drift: `cargo run -p notio-fhir-codegen -- emit` then the drift
check. Conformance is measured against Snowstorm as the reference server (see
`docs/architecture.md` §Verification).

## Conventions

- Crate boundaries mirror the layers; dependencies point downward (app/tools →
  crates). Consume the generated `notio-fhir` types directly — never re-model or
  re-serialize FHIR by hand.
- Two disciplines by layer: `notio-fhir` = generated (change the emitter,
  regenerate, never hand-edit `// @generated`); everything else = idiomatic Rust
  of our own design, the FHIR/SNOMED specs as the authority.
- `thiserror` in libraries, `anyhow` only in the binary. No `unwrap`/`expect`
  outside tests. No `use X as Y` import renaming — direct names only.
- Async-first: the server is I/O-bound; idiomatic tokio/axum.

## IMPORTANT hard rules

- **The FHIR + SNOMED specifications are the oracle.** Implement and review every
  spec-facing behaviour (the terminology operations, ECL, RF2 semantics,
  subsumption, value-set expansion) against the vendored FHIR definitions and the
  SNOMED CT / ECL specifications — never from memory. Cite the spec (FHIR
  operation page, ECL grammar section) for conformance-relevant decisions.
- **Cite only durable references:** the FHIR specification, the SNOMED CT / ECL
  specifications, or official external documentation (the Rust book/reference,
  docs.rs of a pinned crate). Never an internal markdown file as a design
  authority; where no spec governs a decision (storage mechanics, the index
  format, infra), flag it: "no spec governs this — our own design".
- **Never hand-edit a `// @generated` file** — change `tools/notio-fhir-codegen`
  and regenerate.
- **Never distribute SNOMED CT content.** SNOMED CT is licensed by SNOMED
  International; the repository ships no RF2 content and no derived edition data.
  A deployment brings its own licensed RF2 release. Test fixtures use shaped,
  synthetic content only — never real SNOMED concepts extracted from a release.
- **The vendored FHIR packages are codegen input, vendored verbatim with
  provenance** (`PROVENANCE.md` per package, a `scripts/vendor/*.sh` fetcher) —
  never hand-edited; change the fetcher and re-run.
- **Comments follow RFC 505 + RFC 1574 with hard budgets** — line comments only,
  pending work is `// TODO(#N):`, settled decisions are `// NOTE:` (a citation +
  one sentence). No essays in code; the record lives on the issue/PR.
- **Branches use conventional types** (`feat/`, `fix/`, `chore/`, `docs/`,
  `refactor/`, `perf/`, `test/`, `ci/`).
- **NEVER add AI/Claude attribution** to commits, PRs, issues, or code — no
  `Co-Authored-By`, no "Generated with", ever.
- **Never weaken, skip, or delete a test** to make a build pass.
- **Build compiling, tested increments** — do not defer compilation; keep every
  crate you touch green.

## Working discipline (`.claude/`)

Path-scoped rules load on demand when files in their scope are read; the rest
apply always. Read the relevant one before working in that area.

- `.claude/rules/rust-style.md`, `reliability.md`, `comments.md`, `testing.md` —
  the Rust engineering discipline (idiomatic style, safety posture, comment
  budgets, test discipline; oracles = Snowstorm/Hermes + the ECL grammar).
- `.claude/rules/spec-adherence.md` — the FHIR + SNOMED/ECL specs are the oracle;
  strictness; cite the spec.
- `.claude/rules/fhir-terminology.md` — FHIR wire conformance (operations,
  versioning, `$expand` paging, `OperationOutcome`, language) — scoped to the
  wire crates.
- `.claude/rules/snomed-terminology.md` — the SNOMED URI standard, implicit value
  sets / concept maps, ECL 2.2, RF2 handling — scoped to the SNOMED crates.
- `.claude/rules/codegen.md`, `vendored-inputs.md` — the `notio-fhir` generation
  discipline and the vendored-input / SNOMED-content-never-committed rules.
- Skills: `/spec-lookup` (find the authoritative spec answer in oracle order),
  `/regen-codegen` (regenerate `notio-fhir` + drift check).
- Agents: `spec-researcher`, `fhir-conformance-reviewer`, `implementer`.

**Spec-oracle precedence:** (1) the FHIR normative spec for the served wire
version, (2) the SNOMED CT URI + ECL specifications, (3) the SNOMED-on-FHIR IG,
(4) the HL7 `fhir-tx-ecosystem-ig`. Reference servers (Snowstorm, Ontoserver,
tx.fhir.org) are behavioural oracles for spec-silent edge cases only.

## References

- @docs/architecture.md — the design authority (the four decisions + citations).
- FHIR terminology module (per version) and the ECL specification are the spec
  oracles; the pinned FHIR packages under `tools/notio-fhir-codegen/vendor/` are
  the codegen input.

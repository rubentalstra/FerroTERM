# CLAUDE.md

**FerroTERM** is a pure-Rust FHIR terminology server for SNOMED CT,
LOINC, and other clinical code systems, SNOMED CT first. It serves the HL7 FHIR
terminology API across R4, R4B, R5, and R6 from one running server, backed by a
memory-mapped concept index, with no JVM and no Elasticsearch. The engine is
code-system-neutral: the operations talk to a code system provider seam, and
each system arrives through its own loader (`docs/architecture.md` §5,
`docs/terminologies.md`). The name is official: Ferro for the Rust family shared
with FerroEHR, TERM for terminology; the site is <https://ferroterm.eu>.

The full design, with citations, is in **`docs/architecture.md`**. Read it
first. This file is the working discipline. Write all prose (docs, comments,
commits, PRs, issues) to `.claude/rules/writing-style.md`.

## The two layers (the FerroEHR split, applied to FHIR + SNOMED)

- **`ferroterm-fhir` is GENERATED** from the vendored, machine-readable FHIR specs
  (`StructureDefinition` and `OperationDefinition`, from the pinned
  `hl7.fhir.*.core` packages). Treat every file marked `// @generated` as
  off-limits. To change output, change the generator (`tools/ferroterm-fhir-codegen`)
  and regenerate; never hand-edit generated code. The generator emits per-version
  modules (R4/R4B/R5/R6) so the operation surface is correct per version by
  construction.
- **The engine is HAND-WRITTEN** and is the product: the code-system-neutral
  memory-mapped concept store, hierarchy graph, and `fst` and `roaring`
  designation index, the per-system loaders (SNOMED RF2 first), ECL
  evaluation, and the FHIR terminology operations over the provider seam.
  Idiomatic Rust of our own design, with the FHIR and SNOMED specifications
  as the authority.

## Repo map (a single Cargo workspace)

`crates/*` = libraries, `app/*` = the server binary, `tools/*` = dev/codegen
tooling not shipped in the server. Every crate carries its own `CLAUDE.md` with
crate-local discipline.

- `crates/ferroterm-fhir`: generated per-version FHIR types and terminology operation
  contracts (`// @generated`).
- `crates/ferroterm-rf2`: SNOMED CT RF2 loader (inferred relationships, descriptions,
  refsets, transitive-closure file) and typed component model.
- `crates/ferroterm-graph`: the materialized hierarchy of a loaded code system.
  Integer-keyed CSR adjacency (is-a and per-relationship-type) and roaring
  transitive-closure bitmaps; subsumption and ECL set algebra.
- `crates/ferroterm-store`: the memory-mapped (`redb`) columnar concept and
  designation store, one per code system version. Point reads for
  `$lookup`/`$validate-code`.
- `crates/ferroterm-text`: the `fst` and `roaring` designation search index
  (per-word prefix, language and use filters, matched-term-length sort).
- `crates/ferroterm-ecl`: the Expression Constraint Language lexer, parser, and
  evaluator (compiles ECL to set algebra over `ferroterm-graph`).
- `crates/ferroterm-terminology`: the engine. `$lookup`/`$validate-code`/`$expand`/
  `$subsumes`/`$translate` over the code system provider seam, dispatched per
  FHIR version.
- `app/ferroterm-server`: the `axum` HTTP server (FHIR endpoints, content
  negotiation, runtime version routing).
- `tools/ferroterm-fhir-codegen`: the generator, from vendored FHIR packages to
  `ferroterm-fhir`.
- `tools/ferroterm-build`: the offline build, from an RF2 release to the
  memory-mapped graph/store/text artifacts, once per edition.
- `tools/ferroterm-testkit`: synthetic fixtures for the test suites (a
  shaped SNOMED edition written the way the build writes it). A
  dev-dependency of any crate's tests, never a runtime dependency, never
  published: the one sanctioned edge from a crate's tests into `tools/`.

Dependencies point one way (app/tools depend on crates); nothing depends upward
into the server. A code system is a graph MODEL served from an
index-materialized store (CSR adjacency and roaring closure), never a graph
database and never live traversal on the hot path. Nothing in the substrates or
the operations is SNOMED-specific; SNOMED semantics live in its loader, ECL,
and its provider. See `docs/architecture.md` for the evidence.

## Code generation: read this first

The FHIR layer is generated. HL7 publishes the whole type system and every
operation as machine-readable resources in versioned packages
(`hl7.fhir.r4.core` 4.0.1, `hl7.fhir.r4b.core` 4.3.0, `hl7.fhir.r5.core` 5.0.0,
`hl7.fhir.r6.core` 6.0.0-ballot), plus `hl7.terminology` (THO). We vendor and pin
those packages under `tools/ferroterm-fhir-codegen/vendor/` with a `PROVENANCE.md`
per package, and generate `crates/ferroterm-fhir` from them. **R4B is the first
generation implemented** (current stable R4-family release, a near-superset of
R4); R5, R4, and R6 follow.

- **Regenerate:** `cargo run -p ferroterm-fhir-codegen -- emit` (types) and the
  operation-contract emit; a `codegen-drift` check regenerates in CI and fails on
  any diff.
- **Never hand-edit a `// @generated` file.** Change the emitter or its override
  map, then regenerate.
- **The generator emits the complete model from the vendored inputs** within its
  declared terminology root-set closure (see `codegen.md`). Never trim output to
  quiet a diff or dodge a build error. If consuming code needs a shape the
  generated crate lacks, fix the emitter; never shadow it with a hand-written
  type.

## Tech stack (pinned in the manifests)

Rust stable, edition 2024, resolver 3, pinned in `rust-toolchain.toml`. No
`unsafe` (`unsafe_code = "forbid"`). The authoritative dependency set is the root
`Cargo.toml [workspace.dependencies]`; add to a crate with `dep.workspace =
true`. The menu:

- **Ontology store/index:** `redb` (pure-Rust, memory-mapped, ACID embedded
  engine, the persistence substrate), `roaring` (transitive-closure bitmaps),
  `fst` (description term dictionary), `petgraph` (in-memory CSR/graph algorithms
  for the offline build). Pure Rust, memory-mapped, disk-backed. Not
  Elasticsearch, not a graph database, not a C storage or search library.
- **HTTP/async:** `axum`, `tower`, `tower-http`, `hyper`, `tokio`.
- **Parsing (ECL):** `logos` (lexer) and `winnow` (parser). Prefer `winnow` over
  `chumsky`, which is still pre-1.0 with churn; the ECL grammar stays faithful to
  the official ANTLR `.g4`. Diagnostics via `miette`/`ariadne`.
- **RF2:** `csv` (RF2 is tab-delimited), streamed.
- **Serde/formats:** `serde`, `serde_json`; FHIR JSON (and XML where a version
  needs it) through the generated codec.
- **Errors:** `thiserror` in libraries, `anyhow` only in the binary. No
  `unwrap`/`expect` outside tests.
- **IDs/time:** `jiff` for FHIR dates and times.
- **Observability:** `tracing`, `tracing-subscriber`.
- **Testing:** `cargo-nextest`, `insta` (snapshots), `proptest`, `reqwest` with
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

Regeneration and drift: `cargo run -p ferroterm-fhir-codegen -- emit`, then the drift
check. Conformance is measured against Snowstorm as the reference server (see
`docs/architecture.md` §Verification).

## Conventions

- Crate boundaries mirror the layers; dependencies point downward (app/tools
  depend on crates). Consume the generated `ferroterm-fhir` types directly; never
  re-model or re-serialize FHIR by hand.
- Two disciplines by layer. `ferroterm-fhir` is generated (change the emitter,
  regenerate, never hand-edit `// @generated`); everything else is idiomatic Rust
  of our own design, with the FHIR/SNOMED specs as the authority.
- `thiserror` in libraries, `anyhow` only in the binary. No `unwrap`/`expect`
  outside tests. No `use X as Y` import renaming; use direct names only.
- Async-first: the server is I/O-bound; use idiomatic tokio/axum.

## IMPORTANT hard rules

- **The FHIR and SNOMED specifications are the oracle.** Implement and review
  every spec-facing behaviour (the terminology operations, ECL, RF2 semantics,
  subsumption, value-set expansion) against the vendored FHIR definitions and the
  SNOMED CT / ECL specifications, never from memory. Cite the spec (FHIR operation
  page, ECL grammar section) for conformance-relevant decisions.
- **Cite only durable references:** the FHIR specification, the SNOMED CT / ECL
  specifications, or official external documentation (the Rust book/reference,
  docs.rs of a pinned crate). Never cite an internal markdown file as a design
  authority. Where no spec governs a decision (storage mechanics, the index
  format, infra), flag it: "no spec governs this, our own design".
- **Never hand-edit a `// @generated` file.** Change `tools/ferroterm-fhir-codegen`
  and regenerate.
- **Never distribute SNOMED CT content.** SNOMED CT is licensed by SNOMED
  International; the repository ships no RF2 content and no derived edition data.
  A deployment brings its own licensed RF2 release. Test fixtures use shaped,
  synthetic content only, never real SNOMED concepts extracted from a release.
- **The vendored FHIR packages are codegen input, vendored verbatim with
  provenance** (a `PROVENANCE.md` per package, a `scripts/vendor/*.sh` fetcher).
  Never hand-edit them; change the fetcher and re-run.
- **Comments follow RFC 505 and RFC 1574 with hard budgets:** line comments only,
  pending work is `// TODO(#N):`, a settled decision is `// NOTE:` (a citation and
  one sentence). No essays in code; the record lives on the issue/PR.
- **Prose follows `writing-style.md`:** no em dashes, no "not X but Y", no
  decorative triads, no filler buzzwords.
- **Branches use conventional types** (`feat/`, `fix/`, `chore/`, `docs/`,
  `refactor/`, `perf/`, `test/`, `ci/`).
- **NEVER add AI/Claude attribution** to commits, PRs, issues, or code: no
  `Co-Authored-By`, no "Generated with", ever.
- **Never weaken, skip, or delete a test** to make a build pass.
- **Build compiling, tested increments.** Do not defer compilation; keep every
  crate you touch green.

## Issue workflow (the loop)

The tracker is GitHub Issues; the open issue list is the worklist
(`.claude/rules/issue-workflow.md`). One type label per issue (bug/enhancement/
documentation/chore/refactor/perf/ci), one priority label (P0-P3), and domain
labels as needed. Milestones are releases. Record progress on the issue (tick
criteria, comment); a PR declares `Closes #N`. New work found while working an
issue is filed and fixed before the next unit starts (fix-first cadence). Native
sub-issue and dependency edges are set only with `scripts/gh/rel.sh`; the roadmap
board is a view over the tracker, written only with `scripts/gh/project.sh`
(`.claude/rules/issue-relationships.md`, `project-board.md`). The SessionStart
hook prints the open issue list.

## Working discipline (`.claude/`)

Path-scoped rules load on demand when files in their scope are read; the rest
apply always. Read the relevant one before working in that area.

- `.claude/rules/writing-style.md`: no AI tells in any prose (the top priority
  for docs and comments).
- `.claude/rules/rust-style.md`, `reliability.md`, `comments.md`, `testing.md`:
  the Rust engineering discipline (idiomatic style, safety posture, comment
  budgets, test discipline; oracles are Snowstorm/Hermes and the ECL grammar).
- `.claude/rules/spec-adherence.md`: the FHIR and SNOMED/ECL specs are the
  oracle; strictness; cite the spec.
- `.claude/rules/fhir-terminology.md`: FHIR wire conformance (operations,
  versioning, `$expand` paging, `OperationOutcome`, language), scoped to the wire
  crates.
- `.claude/rules/snomed-terminology.md`: the SNOMED URI standard, implicit value
  sets and concept maps, ECL 2.2, RF2 handling, scoped to the SNOMED crates.
- `.claude/rules/codegen.md`, `vendored-inputs.md`: the `ferroterm-fhir` generation
  discipline and the vendored-input / SNOMED-content-never-committed rules.
- `.claude/rules/ci-cd.md`, `ai-code-review.md`: the CI/CD and supply-chain
  discipline (SLSA L3, signed SBOM, pinned actions) and the advisory-Sonar
  policy.
- `.claude/rules/issue-workflow.md`, `issue-relationships.md`, `project-board.md`:
  the tracker work-style.
- Skills: `/spec-lookup` (find the authoritative spec answer in oracle order),
  `/regen-codegen` (regenerate `ferroterm-fhir` and drift-check), `/next-task`,
  `/phase-done`, `/phase-status` (the issue loop).
- Agents: `spec-researcher`, `fhir-conformance-reviewer`, `implementer` (all on
  Opus 5).

**Spec-oracle precedence:** (1) the FHIR normative spec for the served wire
version, (2) the SNOMED CT URI and ECL specifications, (3) the SNOMED-on-FHIR IG,
(4) the HL7 `fhir-tx-ecosystem-ig`. Reference servers (Snowstorm, Ontoserver,
tx.fhir.org) are behavioural oracles for spec-silent edge cases only.

## References

- @docs/architecture.md: the design authority (the four decisions and citations).
- @docs/implementation.md: the scope checklist. The project has no fixed version
  scope; this list is what the server must do, and release contents are decided
  during development. See `.claude/rules/writing-style.md` §Scope framing.
- @docs/ci-cd.md: the CI/CD and supply-chain design.
- @docs/VERSIONS.md: the pinned version matrix.
- `website/book`: the mdBook documentation site (`website/book/src`).
- `website/landing`: the landing page.
- The FHIR terminology module (per version) and the ECL specification are the
  spec oracles; the pinned FHIR packages under `tools/ferroterm-fhir-codegen/vendor/`
  are the codegen input.

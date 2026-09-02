# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Maintenance rule: every pull request that changes user-visible behaviour (the
REST/terminology surface, ECL, validation, configuration, CLI, or
container/deployment artifacts) adds an entry under **[Unreleased]** in the
same PR. Cutting a release renames [Unreleased] to the version + date and adds a
fresh link reference.

## [Unreleased]

### Added

- Request-scoped resources: `tx-resource` parameters on every operation carry
  `CodeSystem` and `ValueSet` resources served for that request only (a
  supplement applies to the system it names; another resource type is
  refused), `POST $cache-control?mode=start` front-loads them under a
  `cache-id` that the `X-Cache-Id` header names on later requests
  (`mode=end` releases it; an idle cache expires after 30 minutes), and
  `GET $versions` answers the FHIR version of the base. Loaded value sets are
  readable (`GET ValueSet/{id}`) and searchable by `url` and `version`. The
  capability statement gains its canonical, `instantiates` the terminology
  server statement, the `ValueSet` interactions and operations, `versions`
  and `cache-control`, and the ecosystem feature extensions; the terminology
  capabilities list the `$expand` parameters the server evaluates.

- `ValueSet/$expand` and `ValueSet/$validate-code` on R4B, type level, over the
  compose layer. A value set is inline (`valueSet`), loaded from a
  `FERROTERM_CODESYSTEMS` directory (`url` and `valueSetVersion`; the greatest
  version is the default), or a provider's implicit form; `include.valueSet`
  resolves through the same store and a cycle is refused. The expansion is
  flat, pages with `offset` and `count`, honours `activeOnly`, `filter`,
  `displayLanguage`, `includeDesignations` with `designation`,
  `includeDefinition`, `system-version`, `check-system-version`,
  `force-system-version`, and `exclude-system`, and echoes every effective
  parameter plus one `used-codesystem` per code system version used.
  Validation answers `result`, `message`, and `display`, infers the system
  when the value set draws on one, checks the display against the
  designations, marks inactive codes with a warning, and appends the
  `system`, `version`, and `code` echo and an `issues` `OperationOutcome` with
  `tx-issue-type` codings, as the terminology ecosystem expects.

- Any code system published as a FHIR `CodeSystem` resource is served:
  `FERROTERM_CODESYSTEMS` names directories of resources (a FHIR package's
  `package/` directory, such as HL7 Terminology 7.3.0, or plain JSON files) in
  R4, R4B, R5, or R6. Nested concepts and `parent`/`subsumedBy` properties
  form the hierarchy when `hierarchyMeaning` is `is-a`; `caseSensitive`,
  `content` (`example` and `not-present` refuse code lookup and enumeration),
  `versionNeeded`, the resource's own `property` and `filter` declarations,
  and the standard `inactive`, `notSelectable`, `parent`, and `child`
  properties are honoured. A `content = supplement` resource adds its
  designations and properties to the system it supplements; a designation in
  the requested language is the display when the system has none.
- `CodeSystem` instance ids carry the system URL when the version does not,
  so two systems at version `1` do not collide.

- The generated FHIR layer covers R4: `hl7.fhir.r4.core` 4.0.1 is vendored with
  provenance and `ferroterm-fhir` gains the `r4` module (131 root-set types,
  the terminology operation contracts, the JSON codec, and the `Parameters`
  conversions), with every terminology resource of the package round-tripping.
- The generated FHIR layer tracks the R6 ballot: `hl7.fhir.r6.core`
  6.0.0-ballot5 (published on packages2.fhir.org) is vendored with provenance
  and `ferroterm-fhir` gains the `r6` module (161 root-set types, the
  operation contracts including the new `tx-resource` parameters, the codec,
  and the `Parameters` conversions). Ballot content: every R6-only behaviour
  is re-verified when R6 publishes.

## [0.0.3] - 2026-09-02

The first release that serves: a built SNOMED CT edition answers `$lookup`,
`$validate-code`, and `$subsumes` on FHIR R4B, from a binary or the container
image, with a quickstart compose file and readable console logs.

### Added

- `ferroterm-build --rf2 <release> --out <dir>`: the offline build from a SNOMED
  CT RF2 Snapshot to the served artifacts. One `redb` store per edition version
  with the concepts, designations, language-reference-set acceptabilities, the
  `parent`, `definitionStatus`, and `module` properties, and every concept-model
  attribute keyed by its concept id; the is-a hierarchy and the designation
  search index in its blob slots; and a `manifest.json` naming the edition and
  version URIs, the release date, the languages, and the counts. Two builds of
  the same release are byte-identical; the Netherlands edition builds in about
  a minute.
- The first served operations on FHIR R4B: `CodeSystem/$lookup`,
  `CodeSystem/$validate-code`, and `CodeSystem/$subsumes` under `/r4b`, by `GET`
  with query parameters or `POST` with a `Parameters` resource, at the type
  level and (for the latter two) on a `CodeSystem` instance; every client input
  error is an `OperationOutcome`. `GET /r4b/metadata` returns the
  `CapabilityStatement` and `?mode=terminology` the `TerminologyCapabilities`
  describing the loaded editions. The server loads the artifact directories
  named by `FERROTERM_INDEX` at start and refuses a missing or damaged one.
- The code system provider seam and the compose layer the operations run on,
  with SNOMED CT as the first provider: identity by edition version URI,
  display by language reference set, the SNOMED-on-FHIR properties, subsumption
  from the closure, and text search from the index.
- The container image `ghcr.io/rubentalstra/ferroterm` (`linux/amd64` and
  `linux/arm64`; the static binary on `distroless/static-debian13`, numeric
  non-root user, listening on `0.0.0.0:8080`), built from the attested static
  binaries with SLSA Build L3 provenance on the index and each platform manifest
  and an SPDX SBOM per platform, all verifiable with
  `gh attestation verify oci://…`.
- A `compose.yaml` quickstart at the repository root, attached to every
  release: it pulls the released image, mounts a built index read-only, binds
  the loopback interface, and runs with every capability dropped and a
  read-only root filesystem. `docker compose up` beside an index is the whole
  install.
- Console logs an operator can read: a startup banner naming the version and
  the maintainer, one line per loaded code system version (id, system, version,
  concepts, languages, path), one line per request (method, route, status,
  latency, the system and code named), and one line on stop.
  `FERROTERM_LOG_FORMAT` chooses `pretty` (colour on a terminal) or `json` (one
  object per line); `auto`, the default, picks by whether stdout is a terminal.
  `RUST_LOG` filters; the HTTP stack's crates default to `warn`.
- The server stops cleanly on `SIGTERM` and `SIGINT`, finishing the requests in
  flight.

### Changed

- Release binaries ship for Linux only (`x86_64` and `aarch64`, gnu and musl):
  the server runs in a container or on a Linux host. The macOS build is
  dropped from the release; a developer on a Mac builds from source.

## [0.0.2] - 2026-09-02

The first published release. It carries the 0.0.1 contents below plus the
release-build fix; the 0.0.1 tag exists but never published (its build jobs
failed at the packaging step) and stays as an unpublished draft.

### Fixed

- The release build packages the server binary again: the binary is named
  `ferroterm`, as the release workflow, the install guide, and the container
  image expect. The 0.0.1 tag never published because the crate produced
  `ferroterm-server` and the packaging step found no file to strip.

## [0.0.1] - 2026-09-02

The first cut: the project foundation, the generated FHIR layer for R4B and
R5, and the first three SNOMED CT engine substrates (RF2 loading, the
subsumption graph, the concept store). Nothing is served yet; the server
binary answers `GET /health` only.

### Added

- The Cargo workspace: the seven engine crates, the `ferroterm-server` binary
  (with a `GET /health` route), and the two tools, with the pinned dependency
  set, the workspace lint table, and the Rust CI lanes active.
- Project foundation: architecture, `.claude/` project configuration (rules,
  agents, hooks, skills, memory), CI/CD + supply-chain scaffolding, the tracker
  work-style, and citation/funding metadata.
- `ferroterm-fhir-codegen` and the generated `ferroterm-fhir` crate: the
  vendored, pinned `hl7.fhir.r4b.core` 4.3.0 and `hl7.fhir.r5.core` 5.0.0
  packages (with provenance), the terminology root-set types per version (133
  for R4B, 154 for R5), the operation contracts (`$lookup`, `$validate-code`,
  `$subsumes`, `$expand`, `$translate`, `$find-matches`, `$closure`) as the
  version's `OperationDefinition` declares them, and a JSON codec that
  round-trips every terminology resource in both packages. A CI drift check
  regenerates and fails on any diff.
- `ferroterm-rf2`: the SNOMED CT RF2 Snapshot loader and typed component model
  (SCTID check digits and partitions, file-name grammar, concepts,
  descriptions, relationships, concrete values, reference sets with typed
  views, module dependencies resolved to an edition URI).
- `ferroterm-graph`: integer-keyed CSR adjacency and roaring transitive-closure
  bitmaps over the inferred is-a hierarchy, the `$subsumes` outcome over them,
  and a versioned artifact layout.
- `ferroterm-store`: the read-only `redb` concept and designation store with
  point reads, precomputed preferred designations per language reference set,
  typed properties, and blob slots for the graph and text artifacts.

### Changed

- The project has its official name, FerroTERM (Ferro for the Rust family it
  shares with FerroEHR, TERM for terminology), with the site at
  <https://ferroterm.eu>. The repository, every crate (`ferroterm-*`), binary,
  environment variable (`FERROTERM_LISTEN`), and document carry the name; the
  Notio codename is retired.
- The engine is code-system-neutral by design: the FHIR terminology operations
  talk to a code system provider seam, SNOMED CT is the first provider, and
  LOINC, UCUM, ICD-10, and the other systems in `docs/terminologies.md` follow
  through the same seam.
- No existing Rust terminology or FHIR crate is a dependency; the README
  records the evaluation and the reasons.

[Unreleased]: https://github.com/rubentalstra/FerroTERM/compare/v0.0.3...HEAD
[0.0.3]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.3
[0.0.2]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.2
[0.0.1]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.1

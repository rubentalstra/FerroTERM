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

- The ICD-10 family: `ferroterm-build --claml <xml|zip> --system <uri>` builds
  a ClaML classification (WHO ICD-10, ICD-10-NL, or any other) and
  `ferroterm-build --icd10cm <dir|zip>...` builds the NCHS ICD-10-CM release
  (the tabular XML and the order file) into the artifact layout, and the
  server serves them from `FERROTERM_INDEX` beside SNOMED CT and LOINC.
  Chapters, blocks, categories, and subcategories are the codes with the
  period the FHIR ICD page requires; the single-parent tree is the graph
  (`classified-with`); titles, inclusion terms, and short descriptions are
  designations; every other note kind is a property and a filter beside
  `kind`, `usage`, and `valid`; ClaML modifiers expand onto their leaves and
  the ICD-10-CM seventh-character codes hang under their stem. The provider
  answers the generic filters over the tree and `$subsumes` from the closure;
  the ICD page defines no filters and no implicit value sets, so there are
  none.
- RxNorm: `ferroterm-build --rxnorm <zip|dir>` builds the full release or the
  Current Prescribable Content into the artifact layout (the RXCUIs with an
  `RXNORM` atom as codes, the `RXNORM` string as the display, every kept atom
  as a designation, `TTY`, `SAB`, `STY`, and the `RXNORM` attributes as
  properties, the `RXNORM` relationships as typed edges both ways), and the
  server serves it from `FERROTERM_INDEX`. The provider follows the FHIR
  RxNorm page: the `STY`, `SAB`, and `TTY` filters, every `REL` code and
  `RELA` label as a filter over the edges (`=`, `in`, `CUI:` or `AUI:`
  values), the `/vs` implicit value set, no subsumption. Only the unrestricted
  sources are kept unless `--rxnorm-sources` names the licensed ones.



## [0.0.5] - 2026-09-03

The LOINC release: a LOINC release builds into the same artifact layout as a
SNOMED CT edition and is served beside it, UCUM ships with the server, and the
hierarchy closure and the designation index move out of the database into
files beside it, which brings the Dutch edition to 589 MiB on disk and 49 s to
build.


### Added

- LOINC: `ferroterm-build --loinc <release zip or directory>` builds a LOINC
  release into the artifact layout (terms, parts, answer lists, and answers as
  codes; the Component Hierarchy by System as the graph; every `Loinc.csv`
  field as a property; long common names, short names, consumer names, part
  and answer texts, and every linguistic variant as designations), and the
  server serves it from `FERROTERM_INDEX` beside SNOMED CT, the manifest
  naming the system. The provider follows the FHIR LOINC page: codes compared
  without case, `LONG_COMMON_NAME` (or a translation) as the display,
  `STATUS = DEPRECATED` inactive, `=` and `regex` on every field, `copyright`,
  `parent`, and `ancestor` over the hierarchy, and the implicit value sets
  `http://loinc.org/vs`, `/vs/[LL id]`, and `/vs/[part code]`.

### Changed

- The artifact layout: `ferroterm-build` writes the hierarchy closure and the
  designation index as `hierarchy.bin` and `text.bin` beside `store.redb`
  instead of as blobs inside it (manifest version 2, store layout 2), which
  removes the large-value pages and their fragmentation from the database.
  Artifacts built before this change must be rebuilt.

## [0.0.4] - 2026-09-03

The multi-terminology release: any FHIR `CodeSystem`, `ValueSet`, and
`ConceptMap` resource is served beside SNOMED CT, the `ValueSet` and
`ConceptMap` operations join the `CodeSystem` ones on R4B, the registry systems
(BCP 47, BCP 13, ISO 3166-1) ship with the server, request-scoped resources
and `$cache-control` make the HL7 terminology ecosystem suite runnable, and it
runs in CI against a committed pass list. The index builds from the release
zip with a tool that ships in the release and the image.

### Added

- UCUM (`http://unitsofmeasure.org`), served without configuration: expressions
  are parsed against the UCUM grammar over the vendored `ucum-essence.xml`
  (2.2, its licence alongside) and reduced to a magnitude over the seven base
  units; every valid expression is a code and its own display, with an English
  name composed from the essence; `canonical` and `property` are properties and
  filters; `$subsumes` answers `equivalent` for the same unit and
  `not-subsumed` otherwise; the implicit value sets `/vs` and
  `/vs/[expression]`; enumeration is refused.

### Added

- The registry systems, served without configuration: BCP 47 language tags
  (`urn:ietf:bcp:47`, the RFC 5646 grammar over the IANA Language Subtag
  Registry; a well-formed tag with an unregistered subtag is not a code), BCP
  13 media types (`urn:ietf:bcp:13`, the RFC 6838 grammar; `registered` and
  `base` filters, `registered = true` enumerates the IANA registry, and
  `$subsumes` decides by parameters, declining a parameter it does not know),
  and ISO 3166-1 country codes (`urn:iso:std:iso:3166`, from Unicode CLDR;
  case-insensitive alpha-2 codes with `alpha3` and `numeric`, `code regex`).
  The IANA and CLDR data is vendored with provenance
  (`scripts/vendor/registries.sh`).
- `ValueSet/$validate-code` decides membership include by include against
  the code itself, so a value set over a system that cannot be enumerated
  validates; `$expand` answers `too-costly` for more than 1000 concepts without
  `count`; every `OperationOutcome` issue carries `details.text` and a
  `tx-issue-type` coding.

- `ferroterm-build --rf2` takes the SNOMED CT release zip as distributed, not
  only the unpacked directory: the `Snapshot/` tree is unpacked to a temporary
  directory removed with the build, and the bytes written equal those of a
  build from the directory. The tool ships in every release tarball beside
  `ferroterm` (with its own CycloneDX SBOM and attestation) and in the
  container image at `/usr/local/bin/ferroterm-build`; `compose.yaml` gains a
  one-shot `build` service under the `build` profile, so the quickstart is
  `FERROTERM_RF2=<release.zip> docker compose run --rm build` then
  `docker compose up`.

- `ConceptMap/$translate` on R4B, type level. A map is inline (`conceptMap`),
  loaded from a `FERROTERM_CODESYSTEMS` directory (`url` and
  `conceptMapVersion`), or chosen by `source` and `target` scope and
  `targetsystem`; the input is `code` with `system`, a `coding`, or a
  `codeableConcept`; `reverse` reads the groups the other way. R4's
  `equivalence` and R5's `relationship` vocabularies reduce to one model, an
  element without a target or with `noMap` answers `unmatched`, and
  `unmapped` rules (`provided`, `fixed`, `other-map`) apply. Each `match`
  also carries `originMap`, `sourceConcept`, `sourceComment`, and `noMap`, as
  the terminology ecosystem expects. `ConceptMap` resources travel in
  `tx-resource` like the others.

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

[Unreleased]: https://github.com/rubentalstra/FerroTERM/compare/v0.0.5...HEAD
[0.0.5]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.5
[0.0.4]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.4
[0.0.3]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.3
[0.0.2]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.2
[0.0.1]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.1

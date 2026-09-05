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

- `FERROTERM_BASE_URL`: the URL clients reach the server at, stated per version as `CapabilityStatement.implementation.url` and in the terminology capabilities. A server behind a reverse proxy answers on an address it never sees, so the endpoint is configured rather than guessed from a socket or a forgeable header; a deployment that names none states no URL. `compose.yaml` gains a `proxied` profile: Caddy terminates TLS in front and the server publishes no host port.
- `GET /metrics`: the Prometheus exposition of the requests answered (a counter and a duration histogram per method, matched route, and status) and the code system versions loaded (one gauge each). The route is off the FHIR base path, so a scrape is never a terminology request, and the labels name the matched route rather than the URI, so the series count stays bounded whatever codes clients ask about.
- `X-Request-Id` on every response: the client's own value when it sent a usable one, a fresh UUID otherwise, and the same id in the request's log line. An id that is empty, over 128 characters, or not printable ASCII is replaced rather than echoed.
- `$batch-validate-code` on `ValueSet` and `CodeSystem`, on every served version: one `POST` carries the shared inputs once, a `validation` parameter per validation, each a `Parameters` of that validation's own inputs, and the answer repeats `validation` in the same order with that validation's `$validate-code` outputs. A validation states its own value for an input the request also states, and a validation the server cannot run answers an `OperationOutcome` in its own slot while the others still answer. No `OperationDefinition` declares the operation anywhere, so the contract is the terminology ecosystem suite's cases, whose `batch/batch-validate` now passes on `/r4b`, `/r4` and `/r5`.
- The fuzz targets under `fuzz/`: every parser an untrusted input reaches (ECL, the FHIR JSON and XML bodies, the SNOMED CT implicit `?fhir_vs=` and `?fhir_cm=` forms, and the RF2 file names, effective times, and identifiers) is fed arbitrary bytes by `cargo-fuzz`, weekly in CI and by hand for longer runs. The seeds are committed; a crash uploads the input that reproduces it.

### Fixed

- A SNOMED CT implicit value set or concept map based on a non-default edition answers when several editions are loaded. The FHIR SNOMED CT page lets any edition version stand as the base URL, and the edition in the base decides the membership, but the server only asked the default version of the system, so every `?fhir_vs` form and `?fhir_cm=` on another loaded edition was refused as a malformed URI. The registry now asks every loaded version, default first, so the bare `http://snomed.info/sct` base still answers from the default edition. A base naming an edition version no loaded version serves is a `not-found` on that version rather than `vs-invalid`.
- The LOINC and RxNorm builds name a property key they never registered instead of reporting "too many concepts": a defect in the build no longer surfaces as a capacity error. The RxNorm relationship-type lookup is split the same way, and `TooMany` keeps its own meaning.
- An expression constraint of deeply nested brackets no longer takes the server down. The ECL parser descends with the grammar, so an unbounded nesting exhausted the stack and aborted the process, which no `OperationOutcome` survives; the parser now refuses past `sct_ecl::NESTING_LIMIT` (64) and the server answers the refusal as an `OperationOutcome`. Found by the `ecl_parse` fuzz target.

### Changed

- A flat `$expand` answers in the order the compose selected: the includes in order, each include's named concepts as it named them, each filter's concepts in the code system's own order, and the first occurrence winning where includes overlap. `count` and `offset` page over that order. No FHIR version fixes the order of an expansion, and `ValueSet.compose.include.concept` says an expansion typically follows the compose; a compose of one filter still pages off the selection bitmap without reading a concept. The ICD-11 postcoordination axis now answers in the WHO's own scale order, which the terminology ecosystem suite's `icd-11/expand-pcs-count` and `icd-11/expand-pcs-offset` cases assert together.

## [0.0.11] - 2026-09-05

### Added

- The build layers derivative reference set packages onto the edition they depend on: `ferroterm-build --rf2 <edition> --rf2-refset <package>` takes the flag repeatably, and each package's concepts, descriptions, and language reference set members join the edition's while its reference sets serve as implicit value sets over it. Every package's module dependency is checked first, so an edition older than the date a package was authored against is refused, naming the module and both dates, and so is a package naming a module the edition does not contain. The manifest records what was layered. The SNOMED CT ICNP Nursing Practice package is the shape this serves.
- `ConceptMap/$closure` maintains a named transitive closure table for a client, over any code system with subsumption: `name` alone creates or empties the table at version `0`, `name` with concepts answers the relationships the client did not have at a new version, and `name` with a version replays everything sent after it. The tables live in the database `FERROTERM_RESOURCES` names and outlive a restart. Naming a table the server was not asked to create is a `404`, and a table whose code systems changed under it answers `422` with `must be reinitialized`. The relationship is read from target to source, and each version states it in its own vocabulary. The R6 ballot ships no `ConceptMap-closure` definition, so `/r6` offers no `$closure` and declares none.
- The batch interaction: `POST [base]` with a `Bundle` of type `batch` runs every entry and answers a `batch-response` with one entry per request, in order, on every served version. A `GET` entry carries the operation's inputs in the query of `request.url` and a `POST` entry in a `Parameters` resource; every terminology operation is reachable, at the type level and at the instance level. The entries are independent: one that fails answers the `OperationOutcome` the same request would have answered on its own, the rest still answer, and the batch answers `200`. A `transaction` Bundle is refused with `not-supported`, and the capability statement declares the `batch` system interaction.
- Persisted `CodeSystem`, `ValueSet`, and `ConceptMap` resources: a deployment that names a database in `FERROTERM_RESOURCES` accepts create, update, read, version read, search by `url` and `version`, and delete on every served version, with the FHIR status codes and the `ETag` and `Last-Modified` headers. `meta.versionId` starts at `1` and rises with every write, `If-Match` makes a write conditional, a deleted resource reads as `410 Gone` and keeps its history. Every operation sees a persisted resource exactly as it sees one loaded from `FERROTERM_CODESYSTEMS`, on every version, and the resources are served again after a restart. A deployment that names no database refuses every write with a `422` and declares no write interaction in its capability statement.
- SNOMED CT implicit concept maps: `ConceptMap/$translate` answers `url=http://snomed.info/sct?fhir_cm=[sctid]` over the reference sets of the loaded edition, on the bare system URI or on the edition or version URI. The four association reference sets the FHIR SNOMED CT page lists (`POSSIBLY EQUIVALENT TO`, `REPLACED BY`, `SAME AS`, `ALTERNATIVE`) each carry the relationship that page gives them, and the map follows the page's template. A map reference set answers from `mapTarget`, with the complex and extended columns (`mapGroup`, `mapPriority`, `mapRule`, `mapAdvice`, `correlationId`, `mapCategoryId`) as `product` parts; no RF2 file records which system a `mapTarget` belongs to, so the group names no target system. A `$translate` that names no map and finds none falls back to the historical associations of an inactive concept, so a retired code answers with its `SAME AS` and `REPLACED BY` successors. Where the specifications are silent, the choice is marked in the code as our own design.
- `$expand` carries the code system's hierarchy: a compose of one include that selects a whole system or an `is-a` subtree nests its `expansion.contains`, with `total` counting every concept and `count` and `offset` paging over the pre-order flattening. `excludeNested` keeps it flat, and so does a text `filter` over a whole system, whose matches have no root to hang from.

### Changed

- The four loader pipelines SonarQube reports as most complex are split along their phases, with no change in behaviour: the LOINC and RxNorm builds and the DHD thesaurus and G-Standaard readers each drop from roughly 250 lines to under 60, and the four `too_many_lines` suppressions they carried are gone. The designation and property-key accumulators become small owned structs rather than closures over the enclosing scope.

## [0.0.10] - 2026-09-04

### Added

- `CapabilityStatement.rest[mode = server].security.service` on every served version, which the terminology ecosystem requires. A deployment names the authentication in front of the server with `FERROTERM_SECURITY_SERVICE` (codes of the FHIR `restful-security-service` value set, comma-separated); the server itself authenticates nobody and says so in text when a deployment declares none.
- `property` on `ValueSet/$expand` on R4 and R4B, pre-adopted from R6 through the ecosystem overlay as the terminology ecosystem requires; the requested properties travel as the R5 cross-version extensions `extension-ValueSet.expansion.property` and `extension-ValueSet.expansion.contains.property`, since those versions have no `property` element. Each version's `TerminologyCapabilities.expansion.parameter` is derived from the parameters its generated `$expand` contract declares, so a version never advertises a parameter it refuses.
- `normalized-code` on both `$validate-code` operations (the terminology ecosystem's output): the code as the system spells it when the request spelled it otherwise, with a `code-rule` note for an alternate form; `code` echoes the request's spelling, on `$lookup` too. `$expand` keeps the compose's spelling of an enumerated code. The capability statements list `property` among the expansion parameters on every version and name the release date of a released build.

### Fixed

- The build warns and omits the release date when `CHANGELOG.md` cannot be read or its heading for the version being built carries no FHIR `date`; a malformed date no longer reaches `CapabilityStatement.software.releaseDate`.
- The compose's spelling of an enumerated code survives an include merge and an `include.valueSet` import in an expansion, and an enumerated concept is located once.
- `excludeNotForUI = true` on `$expand` drops the abstract (`notSelectable`) groupers and `excludePostCoordinated = true` drops post-coordinated expressions; both were accepted and ignored.
- A code a case-insensitive system locates under another spelling is noted as `information` on `$validate-code`, the terminology ecosystem's severity. The message id every issue carries on the wire is decided where the issue is raised, never read off its wording.
- ICD-11: a codeless grouper keeps its abstract status for expansions (`contains.abstract`) and for `$validate-code` with `abstract = false`; only the `$lookup` output `abstract` is withheld, as the ecosystem's icd-11 case expects. A bare ICF code no longer counts as the dotted qualifier form.
- ClaML: a code earns the period after its third character only when its three-character category is one of its ancestors, so the ICD-O morphology class `M953` under `M` keeps its spelling beside the ICD-10 subcategory `M95.3`; the ICD-10-NL 2021 release builds.
- ICD-11: `postcoordinationValues` subproperties carry each value under its own code; an unknown ICF dotted qualifier is an unknown code; a codeless entity answers `notSelectable` as a property, not `abstract`. The `icd-11` suite mode passes 41 of 52 cases (from 28).

### Changed

- The committed terminology ecosystem pass lists grow to 499 general cases on `/r4b`, 500 on `/r4`, and 505 on `/r5` (from 480, 481, and 503), the R4-family `property` and the ICD-11 changes behind the gains.
- The workspace lint table is FerroEHR's: `clippy::all` and `clippy::pedantic` at `deny`, `as_conversions`, `pub_use`, `dead_code`, `missing_assert_message`, `map_err_ignore`, `unused_qualifications`, `rc_buffer`, `create_dir`, `exit`, the feature-name lints, and `non_ascii_idents = forbid` among the additions. Every finding is fixed at its site: an error that wrapped a conversion or parse failure now carries it as its source or states why the cause adds nothing, and the generated `fhir-types` crate carries `unused_qualifications` and `map_err_ignore` in its allow list because every path is spelled from the crate root and a primitive that fails to parse is reported by its path and kind.
- The licence of the project's own code is the Business Source License 1.1
  (`LICENSE`, `NOTICE`): free for non-production use and
  for non-commercial production use, a commercial licence for any other
  production use, and Apache License 2.0 four years after each version. Every
  header, manifest, badge, page, and image label names it, and the versions
  guard fails on a stale Apache 2.0 claim (#221). The `fhir-types` and `rf2`
  crates stay Apache License 2.0 on crates.io, so the FHIR types and the RF2
  reader remain usable by any Rust project (#223).

## [0.0.9] - 2026-09-04

The wire release: FHIR R4, R5, and the R6 ballot are served beside R4B from
the generated per-version modules, the terminology ecosystem's requirements
are an overlay on every version, the tx-ecosystem general mode passes 500 of
670 cases on R5, every route speaks FHIR XML as well as JSON through a codec
generated from the same definitions, and the crates are published on
crates.io under plain names.

### Added

- FHIR R6 (6.0.0-ballot5) is served under `/r6`, marked ballot-tracking in
  both capability statements; the R5 and R6 wires share one family of
  modules, and the ballot's `manifest`, `filterProperty`, and
  `handle-unclosed-expansion` are refused as `not-supported`, never absorbed.
- FHIR XML on every route: `_format` or `Accept` selects
  `application/fhir+xml` for responses, `Content-Type` names an XML
  `Parameters` request body, and both go through `fhir-types::xml`, a codec
  driven by a per-version element schema the generator emits, so the JSON
  codec's strictness applies to both wires. The capability statements list
  both formats.
- The terminology ecosystem overlay: every version's operation contract
  carries the parameters the HL7 terminology ecosystem requires beyond its own
  `OperationDefinition`, pre-adopted from R6 where the ballot declares them
  (the value set and system version trios, `inferSystem`,
  `lenient-display-validation`, `valueset-membership-only`, `useSupplement`,
  the validated `code`, `system`, `version`, `issues`, and `codeableConcept`,
  the `$translate` `source*` and `target*` names and `originMap`) and
  declared by the generator otherwise (`x-caused-by-unknown-system`,
  `x-unknown-system`, `inactive`, `status`, `activeOnly`,
  `used-conceptmap`, `used-system`, the `$translate` match parts
  `sourceConcept`, `sourceComment`, `targetComment`, `noMap`), each marked by
  source in the generated descriptor and in the capability statement.
- Validation answers the ecosystem's shapes: `issues` with the
  `tx-issue-type` codings and `operationoutcome-message-id` extensions, the
  message rule, the `CodeableConcept` shape, version disagreements resolved
  against the value set's version, wildcard system versions, supplements
  applied only when asked, `inactive` and `status` outputs, status-check
  notes for draft, experimental, deprecated, and withdrawn resources,
  deprecated and withdrawn concepts and designations, a value set's own
  language as the display language, whitespace-only display differences,
  and `activeOnly` and membership-only validation.
- `$expand` lists every designation when none is named, filters them by
  `urn:ietf:bcp:47|<lang>`, applies a value set's `valueset-expansion-parameter`
  defaults, populates `contains.version` when a system appears at several
  versions, anchors the `regex` filter, and flags inactive concepts with their
  status.
- `$translate` answers every match with `originMap` and the ecosystem's
  parts, treats a `noMap` element and a `not-related-to` target as answers,
  reports chained (`other-map`) maps as `used-conceptmap`, and reads a
  `target*` input in reverse on every version.
- The Nederlandse Labcodeset: `ferroterm-build --labcodeset` reads the
  publication (the new `labcodeset` crate) and writes the value set over LOINC,
  the LOINC supplement with the Dutch names and the publication's facts, and
  the ordinal outcome value sets as FHIR resources for `FERROTERM_CODESYSTEMS`.
- Conformance badges per served version from the committed pass lists, and a
  Sonar project key that follows the rename to FerroTERM.
- FHIR R5 (5.0.0) is served under `/r5` with the shapes R5 declares: the
  validated `code`, `system`, `version`, and itemised `issues` on both
  `$validate-code` operations, `definition` on `$lookup`, `property` and
  `useSupplement` on `$expand` (with `expansion.property` and
  `contains.property`), and the R5 `$translate` parameter names
  (`sourceCode`, `targetCode`, `relationship`, `originMap`). The R4 and R4B
  endpoints now emit only the outputs their own OperationDefinitions declare.
  The tx-ecosystem suite runs against `/r5` in CI too
  (`conformance/tx-ecosystem/passing-r5.txt`).
- FHIR R4 (4.0.1) is served under `/r4` from the generated `fhir_types::r4`
  module: every terminology operation with R4's own `OperationDefinition`
  parameter set, `GET /r4/metadata` and `?mode=terminology` as R4 resources,
  `$versions`, `$cache-control`, and the `ValueSet` read and search. One set
  of macros instantiates the wire per version, so R4 and R4B cannot drift; a
  cache started on one version serves the other. The tx-ecosystem suite runs
  against `/r4` in CI too (`scripts/checks/tx-ecosystem.sh --fhir r4`,
  `conformance/tx-ecosystem/passing-r4.txt`).

### Changed

- The `crates/*` members are published on crates.io under plain names
  (`fhir-types`, `rf2`, `concept-graph`, `concept-store`, `designation-index`,
  `sct-ecl`, `fhir-terminology`, `loinc`, `classification`, `dhd-thesaurus`,
  `gstandaard`, `labcodeset`, `icd11`, `rxnorm-rrf`) on their own lockstep
  crate line, bumped with their packaged content and guarded in CI, and
  published through crates.io Trusted Publishing from the release and a
  dispatch lane.
- A parameter declared as a primitive reads the primitives that specialize it
  (a `code` for a `string`, a `canonical` for a `uri`), derived by the
  generator from the FHIR type hierarchy.
- The licence of the project's own code is the Apache License 2.0 (`LICENSE`,
  `NOTICE`), replacing MIT; every SPDX header, manifest, badge, image label,
  and page that named MIT follows. Releases up to v0.0.8 stay under the MIT
  terms they were published with. (A Business Source License change merged and
  was reverted the same day, before any release carried it.)

## [0.0.8] - 2026-09-03

The ECL release: the SNOMED CT Expression Constraint Language 2.2, parsed
from the official grammar and evaluated as set algebra over the closure, the
attribute graph, and the reference set tables, behind `?fhir_vs=ecl/` and the
`constraint` filter; the artifact gains the attribute, member, and identifier
files the evaluator reads; LOINC's class parts and axes are served as the
release defines them.

### Added

- ECL: `sct-ecl` parses the Expression Constraint Language 2.2 with a
  lexer and a parser that follow the official ANTLR grammar rule for rule
  (<https://github.com/IHTSDO/snomed-expression-constraint-language>, the tag
  pinned in `docs/VERSIONS.md`, vendored with the example corpus by
  `scripts/vendor/ecl-grammar.sh`). The syntax tree names the grammar's
  constructs; its `Display` prints a canonical form that parses back to the
  same tree; a malformed expression is a typed error with the byte offset and
  the token class expected there. Evaluation follows in the next change.
- SNOMED CT: `ferroterm-build --rf2` writes three more files beside the store
  for the ECL evaluator: `attributes.bin` (every active inferred relationship
  that is not is-a, with its role group and its concept, number, or string
  value, plus an inverted index by type and value), `members.bin` (the active
  member rows of every concept-referencing reference set with their fields,
  the OWL axiom reference sets left out), and `identifiers.bin` (the alternate
  identifiers of the RF2 identifier file). The provider opens them when the
  manifest names them; an older artifact still opens.
- ECL: `sct-ecl` evaluates a parsed expression constraint against an
  edition as set algebra over the closure, the attribute graph, and the
  reference set tables (`sct_ecl::eval`): every constraint operator
  (`<`, `<<`, `<!`, `<<!`, `>`, `>>`, `>!`, `>>!`, `!!>`, `!!<`), member of
  with field selection, refinements with attribute and group cardinalities,
  the reverse flag, dotted attributes, concrete values, conjunction,
  disjunction, exclusion, the description, concept, and member filters, the
  history supplements, and alternate identifiers. The SNOMED provider is the
  edition behind it; an unknown identifier, scheme, field, or dialect alias
  is a typed error, and a construct the artifact cannot answer (a description
  module or effective time filter, a filter on inactive members) is refused
  as unsupported.
- SNOMED CT: `?fhir_vs=ecl/[ecl]` (the expression URI-encoded, on the
  system, edition, or version URI) and the `constraint` filter of
  `ValueSet.compose` evaluate the expression constraint
  (<https://hl7.org/fhir/R4B/snomedct.html>); the `expressions` filter accepts
  `false` and refuses `true` as not supported. Malformed ECL is an
  `OperationOutcome` `invalid` with the byte offset, an identifier the
  edition lacks is `code-invalid`. Parsed expressions are cached.

### Fixed

- LOINC: the class parts that only the Component Hierarchy by System names
  (42,554 in 2.83, such as `LP442038-8 |Bacteria | Abscess | Microbiology|`)
  are concepts with the hierarchy's text as their name, so `concept is-a` and
  `ancestor` over a part reach the terms under it (they were dropped with their
  terms before). The six axes (`COMPONENT`, `PROPERTY`, `TIME_ASPCT`,
  `SYSTEM`, `SCALE_TYP`, `METHOD_TYP`) carry the part the primary part links
  name, as the FHIR LOINC page types them; a filter on an axis matches the
  part's code or its name with `=` and `in`, and its name, code, or column
  text with `regex`. `ferroterm-build --loinc` reads `LoincPartLink_Primary.csv`
  from the zip.
- LOINC: a part displays its `PartName` (`PANEL.HL7.CYTOGEN`), as the
  reference servers do where the FHIR page names no display; `PartDisplayName`
  follows as a synonym.
- `TerminologyCapabilities.codeSystem.version.language` lists the designation
  languages as `CommonLanguages` codes (a tag outside the set by its primary
  subtag, or left out): R4B binds the element to nothing, but the FHIR
  validator converts the resource to R5, whose binding is required, and
  stopped on `ar-JO` from the LOINC linguistic variants. `$lookup`
  designations keep every tag.
- LOINC: the release zip's `AccessoryFiles/PanelsAndForms/Loinc.csv` is no
  longer mistaken for the term table, and a published code whose check digit
  does not follow the Mod 10 algorithm (`11491-6`, deprecated, in 2.83) is
  read as the table lists it; the check digit still applies to codes a client
  submits.

## [0.0.7] - 2026-09-03

The code systems release: the ICD-11 code systems from the WHO ICD-API local
deployment, ATC/DDD, the DHD thesauri, and the G-Standaard product ladder join
the served systems; SNOMED CT gains its implicit value sets; `$expand` pages
over selection bitmaps before reading a concept; `Accept-Language` selects the
display language; and the README, the landing page, and the book describe the
shipped server.

### Added

- SNOMED CT: the implicit value sets `?fhir_vs` (every concept),
  `?fhir_vs=isa/[sctid]`, `?fhir_vs=refset`, and `?fhir_vs=refset/[sctid]` on
  the system, edition, or version URI (<https://hl7.org/fhir/R4B/snomedct.html>);
  `?fhir_vs=ecl/` is refused with an `OperationOutcome` until the ECL
  milestone. `concept in [sctid]` is reference set membership, as the page
  defines it. `ferroterm-build --rf2` writes the active concept members of
  every concept-referencing reference set to `refsets.bin` beside the store
  (an artifact without the file still opens, with no reference sets).
- ICD-11: `ferroterm-build --icd11 <cache> [--icd11-api <url>]` walks a local
  deployment of the WHO ICD-API into a cache of entity JSON and builds the
  MMS, the ICF, and the Foundation as three code systems
  (`http://id.who.int/icd/release/11/mms`, `.../icf`,
  `http://id.who.int/icd/entity`), served from `FERROTERM_INDEX`. A code is a
  short code, an entity URI in either form, or a postcoordination expression
  validated against the stem's axes, with `stem` and `postcoordinationValues`
  in `$lookup`; `id`, `parent`, and `child` are URIs; an entity without a
  short code is `notSelectable`; a stem's scale is an implicit value set
  (`<uri>/postcoordinationScale/<axis>`). `$lookup` properties now carry
  `description` and `subproperty` parts, and a `uri` value.
- The `Accept-Language` header selects the display language of `$lookup`,
  `$validate-code`, and `$expand` when the request names no
  `displayLanguage`; the parameter wins when both are given. Both carry a
  language range list (`en, en-AU; q=0.4`, `de,*`), resolved against the
  languages the code system carries by quality, then position; `*` is the
  system's own language.
- ATC/DDD: `ferroterm-build --atc <index.csv|BST801T> --atc-version <year>`
  builds the WHO index (exported as CSV) or the G-Standaard `BST801T` file
  into the classification layout under `http://www.whocc.no/atc`: the five
  levels as `kind`, the tree from the code prefixes, the names as
  designations by language, every DDD a `ddd` property.
- DHD thesauri: `ferroterm-build --dhd <delivery.zip|dir> [--dhd-version <v>]`
  builds a Diagnosethesaurus or Verrichtingenthesaurus delivery
  (Uitleverformaat 5.0 CSV tables) into a flat classification under
  `urn:oid:2.16.840.1.113883.2.4.3.120.5.1`: the terms as designations by
  type and language, the SNOMED CT identifier, the ICD-10, DBC, and ZA
  derivations, roles, code mappings, replacements, splits, and umbrella terms
  as properties, ended concepts inactive. The build writes
  `conceptmaps/dhd-to-snomed.json` and `dhd-to-icd10.json` (FHIR R4B
  `ConceptMap`) for `FERROTERM_CODESYSTEMS`. A classification without a tree
  (no `hierarchyMeaning`) is served without `parent`/`child` properties and
  without subsumption.
- G-Standaard: `ferroterm-build --gstandaard <dir> --gstandaard-version
  <release>` builds the product ladder from the fixed-length files (`BST711T`,
  `BST052T`, `BST031T`, `BST004T`, names through `BST020T`, thesauri through
  `BST902T`) into four flat classifications under `<out>/{gpk,prk,hpk,artikel}`
  (`urn:oid:2.16.840.1.113883.2.4.4.1`, `.10`, `.7`, `.8`): the full name as
  display, short and label names as designations, the rungs above a concept,
  the ATC code, substance, strength, form, route, brand, and firm as
  properties; removed records skipped, articles with a removal date inactive.

### Changed

- `ValueSet/$expand` pages before it reads: includes and excludes are bitmap
  algebra per code system version, `total` is the bitmap count, and only the
  `count` members after `offset` are read from the store. The order within a
  system is the provider's concept order (the ordinal the build assigns from
  sorted codes; a FHIR `CodeSystem` resource's concepts are numbered in code
  order too) instead of a string sort of the codes, so a page over 133,736
  SNOMED CT descendants answers in 0.6 ms instead of a second, the whole Dutch
  edition in 3 ms instead of four seconds. `activeOnly` over a large set
  subtracts the provider's inactive set, which SNOMED CT reads once from the
  store (0.4 s) and keeps.

## [0.0.6] - 2026-09-03

The ICD-10 and RxNorm release: a ClaML classification (WHO ICD-10, ICD-10-NL,
or any other) and the NCHS ICD-10-CM release build into the artifact layout
and are served with the FHIR ICD page's conventions, and an RxNorm release
(the full release or the Current Prescribable Content) is served with the five
FHIR filters over typed relationship edges.


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
  provenance and `fhir-types` gains the `r4` module (131 root-set types,
  the terminology operation contracts, the JSON codec, and the `Parameters`
  conversions), with every terminology resource of the package round-tripping.
- The generated FHIR layer tracks the R6 ballot: `hl7.fhir.r6.core`
  6.0.0-ballot5 (published on packages2.fhir.org) is vendored with provenance
  and `fhir-types` gains the `r6` module (161 root-set types, the
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
- `fhir-codegen` and the generated `fhir-types` crate: the
  vendored, pinned `hl7.fhir.r4b.core` 4.3.0 and `hl7.fhir.r5.core` 5.0.0
  packages (with provenance), the terminology root-set types per version (133
  for R4B, 154 for R5), the operation contracts (`$lookup`, `$validate-code`,
  `$subsumes`, `$expand`, `$translate`, `$find-matches`, `$closure`) as the
  version's `OperationDefinition` declares them, and a JSON codec that
  round-trips every terminology resource in both packages. A CI drift check
  regenerates and fails on any diff.
- `rf2`: the SNOMED CT RF2 Snapshot loader and typed component model
  (SCTID check digits and partitions, file-name grammar, concepts,
  descriptions, relationships, concrete values, reference sets with typed
  views, module dependencies resolved to an edition URI).
- `concept-graph`: integer-keyed CSR adjacency and roaring transitive-closure
  bitmaps over the inferred is-a hierarchy, the `$subsumes` outcome over them,
  and a versioned artifact layout.
- `concept-store`: the read-only `redb` concept and designation store with
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

[Unreleased]: https://github.com/rubentalstra/FerroTERM/compare/v0.0.11...HEAD
[0.0.11]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.11
[0.0.10]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.10
[0.0.9]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.9
[0.0.8]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.8
[0.0.7]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.7
[0.0.6]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.6
[0.0.5]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.5
[0.0.4]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.4
[0.0.3]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.3
[0.0.2]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.2
[0.0.1]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.0.1

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

## [0.1.3] - 2026-09-12

### Added

- `ferroterm healthcheck`, a subcommand that sends one `GET /health` to the
  server's own listen address and exits 0 on `200 OK`, 1 otherwise with the
  reason on stderr. The distroless image has no shell or HTTP client, so this
  is what its new `HEALTHCHECK` runs, and the shipped `compose.yaml` states the
  same block: `docker compose up --wait` now waits for a serving instance, and
  another service sequences itself after the server with
  `depends_on: ferroterm: condition: service_healthy`, as the shipped `proxy`
  service now does. Because the server binds only after every artifact is
  open, a passing probe means the deployment answers terminology requests
  (#565).

### Changed

- The FHIR model now comes from crates.io. `fhir-types` and the generator that
  emits it, with the five vendored HL7 packages behind it, moved to the
  FerroBRIDGE repository, which publishes the crate; FerroTERM depends on it by
  version like any other dependency (#300). Consumers of the crate are
  unaffected: the same crate line continues on crates.io, published from
  FerroBRIDGE, and the published 0.1.98 source is byte for byte what this
  repository generated at 0.1.97. What leaves here is the generator, its
  vendored input, the `codegen-drift` CI job, and the `fhir-types` entry of the
  publish order. What replaces the drift job is a pin check:
  `scripts/checks/versions.sh` fails when the requirement in `Cargo.toml`, the
  row in `docs/VERSIONS.md`, and the version in `Cargo.lock` disagree, so a new
  release reaches the build only in a change that records it, and Dependabot
  opens that pull request on its own.

## [0.1.2] - 2026-09-12

### Added

- The viewer has a design system, and every screen is drawn from it. Colour is named by role on `:root` and redefined once under `:root.dark`, so a theme decision is never taken per element; type is six steps, spacing four on an 8-point grid, and motion one duration. A screen names a role and a step and nothing else, which `scripts/checks/viewer-tokens.sh` enforces as its own CI job: a palette entry, a per-element `dark:` variant, a raw type size, an off-scale spacing value, or a bracketed value fails the build. The point is measurement rather than tidiness: a pairing defined once is checked once, and the WCAG 2.2 AA contrast pass covers every pairing the viewer draws in both themes and both densities.

- A command bar sits above every screen, and the navigation is four labelled groups. One field takes a code, a canonical, or free text, decides which it is from the string's shape before any request, and offers what this root can do with it; every offer is a real link, so the keyboard contract is the browser's own and an offer can be shared. The screens are grouped by what a reader came to do rather than by resource type.

- Every runner picks its code system rather than waiting for a canonical to be typed. `GET /{version}/{type}?_elements=url,name,title,version` is about four kilobytes where the whole search is a quarter of a megabyte, so each control offers what this root publishes, the name leading and the canonical as the value, with the versions beside it being the ones that canonical was published in. A canonical the root does not publish is still typed, and the control opens on the text field for one rather than snapping back to a list it is not in. The screens are named for the question rather than the operation that answers it: Check a code, List a value set, Map a code.

- The overview reads a twenty-system deployment on one screen. It is one table rather than a card each, sortable by any system-level column, each row leading with the name its code system was published under and the canonical beside it, quieter and still selectable. Absence is a mark with the sentence behind it rather than a sentence in every cell. The top bar and the sidebar stay put while the screen scrolls under them.

- `_elements` is answered on every served version. The parameter is defined by every release this server speaks (`http://hl7.org/fhir/SearchParameter/Resource-elements`) and was refused as unsupported; a search now returns the elements it names plus the ones every resource keeps, and marks what it sends `SUBSETTED` so a client cannot store a subset back as the whole resource. Reading eleven code systems by name costs 4 KB where the whole search costs 255 KB.

- SNOMED CT's `normalForm` and `normalFormTerse` are generated and served. Both are the Necessary Normal Form of a concept, which the FHIR SNOMED CT page defines and does not say how to produce. The generation is a rendering rather than a computation: the necessary normal form of a precoordinated concept is what the release's inferred view already holds (the SNOMED CT Release File Specification, Appendix D), so the focus concepts are the concept's inferred `is a` parents and the attributes are its other inferred rows, and no reasoner and no supertype walk runs. The syntax is the Compositional Grammar specification §5, the definition status prefix its §6.7: `===` where the RF2 `definitionStatusId` says the concept is sufficiently defined, `<<<` where it does not, both always written because the grammar assumes `===` when none is. Three shapes are this server's own, because no specification fixes them, and each is recorded: the order of what is rendered, which term a reference carries, and the layout. A request naming no property is answered as before, which is what "the server chooses what to return" admits.

- An openEHR archetype's local terminology is served through the ordinary operations. An archetype constrains most of its coded fields with an archetype-local `at`-code list, and nothing publishes those as FHIR resources. The scope decision is recorded: this server ingests the resources a producer derives from an archetype and reads no openEHR artefact, so it keeps no openEHR dependency. The `at`-codes as a `CodeSystem`, an `ac`-code as a `ValueSet`, and `term_bindings` as a `ConceptMap` load through `FERROTERM_CODESYSTEMS` like any other supplied resource, and `$lookup`, `$expand`, `$validate-code` and `$translate` answer over them with no openEHR-specific request shape. No openEHR specification defines a canonical URI for a local value set, so whoever produces the resources mints the URL from the archetype id, and this server reads it as an opaque canonical: two deployers minting the same archetype under their own domains are served side by side, and two directories naming one canonical at one version refuse to load rather than picking a winner.

- The brand tokens state what each is safe for. Three of them are safe for the mark and not for text, and nothing said so: teal reaches 3.58:1 on the light surface, which clears the 3:1 a graphic needs and falls short of the 4.5:1 a label needs. Each token carries its measured ratio against both grounds, `--ferroterm-brand-text` is the brand colour as text (deep teal on light, teal on the tile, each at 4.5:1 or better), and `scripts/checks/brand-contrast.sh` fails when a token's own comment or the published table stops matching what the value measures. The mark is unchanged.

### Fixed

- An unresolvable `exclude.valueSet` is named rather than passed over. `ValueSet/$validate-code` walked the excludes only after an include had held the code, so an exclude naming a value set this server does not hold was never reached and the answer was a silent `result = false` over a definition it could not fully evaluate. The excludes are walked either way now, which changes what is reported and no answer: an exclude removes codes, so a `false` it could not evaluate was never going to become a `true`. The terminology ecosystem settles the reached case as an answer rather than a refusal, with the canonical named in an issue of `tx-issue-type#not-found`, and it requires `$expand` and `$validate-code` to differ in shape over one such value set; a refusal stays reserved for a definition that cannot be processed at all, which is a cycle.

- A cycle names the value set that closed it. The reference chain pushed the root as `url|version` and every reference as the bare reference string, so the two spellings could never match: a value set referencing itself was caught one hop late, and a two-hop loop was reported under the value set it reached rather than the one that closed it. One rule now decides whether a reference re-enters a chain entry, comparing the url and the versions where both name one, because a reference naming no version means the version the server resolves to.

- `check-system-version` is compared against the version an include resolved to. The check ran against the version or pattern a value set wrote, before that version had been resolved against what the server serves, so a value set naming `1.x.x` was reported as failing a `1.0.x` check under the pattern's own spelling rather than under the `1.2.0` it selects, and a value set naming a version nothing serves was reported as failing the check rather than as naming a version that does not exist. The pin now applies the defaults and the forces alone and the checks run over the versions the expansion actually used, so a version that resolves to nothing fails as unknown first, which is what it is. An answer that says a version is unknown also names the versions the server does hold.

## [0.1.1] - 2026-09-08

### Added

- The viewer runs `$validate-code` and `$subsumes`. `/ui/validate` validates one code against a code system or a value set, and asks how two codes of one system relate. What the form offers is the root's own capability statement: a resource type it does not declare `$validate-code` on is not offered, and where a root declares the instance level a stored resource's id stands in for the canonical. The answer states the verdict, the message the server gave, the display the system holds, the version it validated in, and whether the concept is inactive, with every issue the server itemised and the codes beside its text. Both runners put their parameters in the address, so a case is a link you can share.

- The viewer compares the four served roots. `/ui/versions` reads `GET /{version}/metadata` from `/r4`, `/r4b`, `/r5`, and `/r6` and draws two tables: what each root says about itself, and which of the terminology operations it declares and at which levels. The screen exists because the four genuinely differ; R4 and R4B declare `$lookup` at the type level and R5 added the instance level, and the viewer offers on every other screen only what the root you are on declares. Each root's own `operation.documentation` is drawn below, which is where a release states what it takes beyond its own `OperationDefinition`.

- The viewer carries the conformance and benchmark evidence this build ships. `/ui/evidence` is the one screen that asks the server nothing: every figure was read out of a committed file by the build script and names that file on the page, so a reader who doubts a number opens it in the repository. It draws the HL7 terminology ecosystem suite mode by mode with the cases each passed of the cases it ran, the latency bars the project claims with the run recorded against each, and the newest committed benchmark run system by system. The figures describe the FerroTERM build the bundle came from, never the deployment answering the page, and the screen says so.

- The viewer's screens sit in a left sidebar, each with its own icon, and the FHIR version selector stays in the top bar. The selector changes what every screen reads rather than moving you to a screen of its own, which is why it is not in the list. On a narrow window the sidebar folds behind a button. The icons are the viewer's own, drawn as inline SVG with no icon package in the bundle.

- The FHIR core code systems and value sets the specification defines are served without configuration. A deployment that loads no artifact still answers `$lookup`, `$validate-code`, and `$expand` for the code systems FHIR publishes itself, so a resource bound to `administrative-gender` or `publication-status` validates against the version of FHIR it was written for.

- Every ISO 3166 code form is a code of `urn:iso:std:iso:3166`. FHIR selects all three from it with a `code` regex per form, `[A-Z]{2}` for `ValueSet/iso3166-1-2`, `[A-Z]{3}` for `-1-3`, and `[0-9]{3}` for `-1-N`, so an alpha-3 or numeric country code the specification binds has to validate. Only the alpha-2 form was a code, and `NLD` and `528` were refused against a system that holds `NL`. Each territory is three concepts now, sharing a display and carrying all three forms as properties.

- The viewer browses the concepts of one code system. `/ui/browse` searches a system, reads one concept with `CodeSystem/$lookup`, and walks the hierarchy the served version declares. What the screen offers is read from `TerminologyCapabilities` alone. A version that declares a filter carrying `child-of`, the operator FHIR defines as "all concept ids that have a direct parent that matches", gets the taxonomy tree on the property that filter is declared on; a version that declares other hierarchy operators, or none at all, gets the flat searchable list with a sentence naming what it does declare. Every search and every level of the tree is a `POST ValueSet/$expand` whose value set travels inline as the operation's own `valueSet` input, so the screen names no implicit canonical and browses any served system the same way; each request asks for `excludeNested` so one level arrives as one list. The tree follows the ARIA tree view pattern: one tab stop that follows the browser's own focus, the arrow keys walking and opening, `Enter` selecting, and each row carrying its level, its position among its siblings, and whether it is expanded. A row is keyed by the path of codes above it, so one concept sitting under two parents draws two rows that never share state, exactly one of them is announced as selected, and an answer that loops back stops at the repeat. A node whose children the server counts higher than it listed says so on the node, so a level cut short by the page size never reads as the whole of it. Every pane renders the request its answer belongs to rather than the address as it stands, so a code is never shown paired with the concept that was read before it. The concept pane draws every designation and property the server answered, and links the parents and children it declares. Every parameter lives in the address, so a concept you are reading, the filter you typed, and the nodes you opened are all one link you can share, and a refusal renders the server's own `OperationOutcome`.

- The viewer lists the concept maps a root holds and translates codes with them. `/ui/conceptmaps` reads `GET /{version}/ConceptMap` the way the value set screen reads its own type, draws every fact a map's publisher declared, and lists the groups it maps with the code count of each. The scope elements are read under both spellings, because R4 and R4B carry `source[x]` and `target[x]` and R5 renamed them `sourceScope[x]` and `targetScope[x]`. The `$translate` runner below appears only where the root's `CapabilityStatement` declares `translate` on `ConceptMap`, so the screen never offers a run the server then refuses. Every parameter is sent under the name that FHIR version's own `OperationDefinition` declares: R4 and R4B take `code`, `system`, `version`, and `targetsystem`, R5 renamed the code to `sourceCode` and the target system to `targetSystem`, and the R6 ballot renamed `system` and `version` to `sourceSystem` and `sourceVersion`. The answer is read the same way: a match's relation is labelled `Equivalence` where R4 and R4B answered one and `Relationship` where R5 and the R6 ballot did, over two different sets of codes, so one version's answer is never drawn as another's. Each match also draws the target `Coding`, the map it came from, the source concept and comments, and any `product`, `dependsOn`, or `property` the answer carried. Matches keep the server's own order, which is the edition's concept order for a reverse translation. Every parameter of a run lives in the address, so a translation is a link you can share, and a refusal renders the server's own `OperationOutcome`.

- The viewer lists the value sets a root holds. `/ui/valuesets` reads `GET /{version}/ValueSet`, filtered by the `url` and `version` search parameters, and reads one of them by id. The filter, the page, and the resource you opened all live in the address, so a list is a link you can share and the back button walks it. This server answers a search in one `searchset` and pages none of it, so the pages the screen walks are its own over the answer that arrived, and it says so. A value set you open draws every fact its publisher declared, the clauses its `compose` selects with, and a link that opens its canonical in the expansion runner. A resource that carries no id says so rather than drawing a link that would answer nothing, and a refusal renders the server's own `OperationOutcome`. The screen only reads: the server answers create, update, and delete on this resource type and the viewer calls none of them.

- The viewer runs `$expand` and walks its pages. `/ui/expand` takes a value set canonical with `filter`, `count`, `offset`, `displayLanguage`, `activeOnly`, and `includeDesignations`, sends each one only when you set it, and draws the concepts with their designations, `expansion.total`, and the parameters the server says it applied. Every parameter lives in the address, so a page is a link you can share and the back button walks the run; an implicit canonical carrying its own query string is percent-encoded into the one parameter it belongs to. An expansion the server marks with `valueset-unclosed` says so above the table, in the server's own words where it states a reason, because a code missing from an unclosed list is not a code the server rejects. A refusal renders the `OperationOutcome` verbatim, with the codes of `issue.details` beside the text, and a `too-costly` refusal adds what to do about it: ask for a page size and the runner walks the selection instead. The screen names no code system.

- The viewer's first screen answers what a deployment loaded. `/ui` reads `GET /{version}/metadata?mode=terminology` and draws one card per code system: the canonical, every served version with the default marked, whether the server reads the system's compositional grammar, the designation languages, the declared filters with their operators, the artifact the version was read from with the release the offline build recorded, the content mode where the FHIR version has that element, and whether subsumption is supported. R4 and R4B declare no `codeSystem.content`, so the card says the version does not carry it rather than showing nothing, and the R6 ballot's `version.value` is read where R4, R4B, and R5 use `version.code`. A system that declares no version, no language, or no filter states the absence, and a system read from no artifact, such as a registry the server carries, says that rather than reading as a gap. The screen names no code system: every card is the capability statement rendered, so a system this server has never served draws with no change to the viewer, and a test proves it by feeding a document naming an invented system. Each section discloses the request it issued, as a URL and as a `curl` line, and renders the server's own `OperationOutcome` inline when a read is refused.

- The server serves the viewer at `/ui`, and `/` redirects onto it. The bundle Trunk builds rides inside the `ferroterm` binary as a table of files the build script writes, so a release tarball and the container image behave the same and a request path never reaches the filesystem. A path under `/ui` the bundle does not hold answers `index.html`, which is how a client-side route deep-links; the fallback is scoped to `/ui`, so an unknown path outside it still answers the `not-found` `OperationOutcome`. Content-hashed assets are served `public, max-age=31536000, immutable` and `index.html` `no-cache`, each with its own media type and `X-Content-Type-Options: nosniff`. Asset requests are outside the request log and the `/metrics` histograms, so the latency figures keep describing the terminology operations. `FERROTERM_UI=off` drops the routes and restores today's `/`, and a binary built without the viewer serves no `/ui` route and says so at start.

- `$expand` over `urn:ietf:bcp:47` answers when the value set fixes the primary language subtag. RFC 5646 §2.1 gives every position of a tag a bounded shape (`language = 2*3ALPHA / 4ALPHA / 5*8ALPHA`, `script = 4ALPHA`, `region = 2ALPHA / 3DIGIT`) and §3.1 registers a finite list of subtags for each, so a fixed language leaves a finite list of tags: the language itself with each registered region, and, once a region is fixed too, that tag with each registered script. A selection that leaves the primary language open still answers `not-supported`, and an unpaged enumeration past the server's own expansion limit answers `too-costly` like any other. The expansion is marked unclosed whatever it lists, because `extension = singleton 1*("-" (2*8alphanum))` and `privateuse = "x" 1*("-" (1*8alphanum))` build unboundedly many tags on any of them, and it now states that as its `valueset-unclosed-reason`.
- Every subtag of a registry range is registered. The IANA Language Subtag Registry writes the private-use blocks as ranges (`qaa..qtz`, `Qaaa..Qabx`, `QM..QZ`, `XA..XZ`), and RFC 5646 §3.1.1 defines the notation: "The sequence '..' (U+002E U+002E) in a field-body denotes a range of values. Such a range represents all subtags of the same length that are in alphabetic or numeric order within that range, including the values explicitly mentioned." The reader took each range as one literal subtag, so `en-QM`, `en-XX`, and the private-use language `qaa` were well-formed but not valid, and `$validate-code` refused them. The registry now holds 8795 language, 274 script, and 343 region subtags.
- An implicit SNOMED CT value set carries the template the FHIR SNOMED CT page prints for it. The page gives a template per `?fhir_vs=` form and says "the content of the resource must conform to the template provided", and each one carries `version`, `name`, `description`, and `copyright` beside the `compose` the server already answered. The `copyright` is the page's own SNOMED International notice, character for character, which tells a consumer the expansion holds licensed content; `name` and `description` interpolate the sctid, the reference set, or the expression constraint as the template shows, with the preferred term where the template says "[sctid or preferred description]". `description` and `copyright` travel with the definition, as `publisher` does, so an expansion without `includeDefinition` is unchanged. The page prints no template for the bare `?fhir_vs`, so that form carries only the two fields every template shares.
- `?fhir_vs=refset` lists the language reference sets the edition defines. The page defines the set as "all concept ids that correspond to reference sets that are explicitly defined in the specified SNOMED CT edition", with no category excluded, and a language reference set is one; their membership reaches the designation index rather than the reference set tables, so the list missed them. The Netherlands edition lists 80 reference sets where it listed 75, the International edition 27 where it listed 25. The member-facing forms stay refused and now say why: a language reference set references descriptions, so "all concept ids in the specified reference set" selects none of them.
- A `version` naming only a SNOMED CT edition resolves to the greatest loaded release of that edition. The page tells clients to send that form, "At minimum the URI SHOULD contain the sctid of the SNOMED CT distribution", and lets a service default, "if the date of release is not provided, the Terminology Service may default to the most recent version of the named SNOMED CT distribution". Every operation that takes a system version reads it through the same resolver. The date-only form stays an error, as the same section asks, and so does an edition no loaded release belongs to.

- An expansion that leaves out codes the value set admits says so: `ValueSet.expansion` carries `http://hl7.org/fhir/StructureDefinition/valueset-unclosed` with `valueBoolean` true. `$expand` describes the case as a value set "unbounded due to the inclusion of post-coordinated value sets (e.g. SNOMED CT, UCUM)". Which systems report it is decided one at a time: BCP 13 media types selected by a filter (each registered type carries unboundedly many parameterised forms), SNOMED CT unless the include states `expressions = false`, UCUM and BCP 47 language tags, and a `CodeSystem` resource whose `content` is `fragment`. An include that names its concepts is complete whatever the system admits, and LOINC, RxNorm, the ICD and ICPC classifications, ICD-11, and ISO 3166 expansions list every member and are unchanged.

- `$expand` on `/r6` takes `handle-unclosed-expansion`, the parameter the R6 ballot alone declares. The ballot states it as a pair of rules: "If true this asserts that you will correctly handle an unclosed expansion and the returned expansion SHALL include the valueset-unclosed extension if the value set is unclosed. If handle-unclosed-expansion is set to false the server SHALL return an error if the value set is unclosed." So `true` is answered with the expansion and its mark, and `false` over an unclosed value set is a 400 `not-supported` `OperationOutcome` instead of an expansion. The ballot fixes nothing for the absent parameter, so a request that names none keeps the mark, which is what R4B and R5 describe. R4, R4B, and R5 declare no such input and refuse the name as undeclared, as they refuse any parameter their `OperationDefinition` does not carry.

- An expansion drawing on a code system fragment says which fragment and why. `ValueSet.expansion` carries `valueset-unclosed-reason` beside `valueset-unclosed`, naming the system whose resource holds only "a subset of the code system", and `expansion.parameter` carries a `used-fragment` per fragment the expansion drew on, beside the `used-codesystem` already answered. The terminology ecosystem asks a server to report "all versions of code systems used (in 'used-*')", and its `fragment/fragment-expansion` case fixes both spellings; that case now passes on `/r4b`, `/r4`, and `/r5`.

### Fixed

- A concept whose `status` property is `deprecated` is no longer reported inactive. The specification's own property definition says the two are different: "Concepts that are deprecated but not inactive can still be used, but their use is discouraged, and they should be expected to be made inactive in a future release" (<https://hl7.org/fhir/R5/codesystem-concept-properties.html>). Only `retired` is read as inactive, which is the value the `inactive` definition points at. An expansion no longer marks a deprecated concept inactive, and `activeOnly` no longer drops it.

- A `displayLanguage` that lands on a designation keeps the system's own display. Where a request asks for a language the concept carries a designation in, that designation becomes the `display`, and the code system's own display had nowhere to go: the answer repeated the promoted designation and dropped the original. It moves into the designation list now, marked preferred for the language the system states, so the answer carries each translation once.

- A display language a request refuses is no longer answered anyway. RFC 9110 section 12.5.4 gives a zero-quality range one meaning, "not acceptable", and `displayLanguage` carries that syntax, so `de, *;q=0` refuses every language the list does not name. Zero-quality ranges were dropped while parsing, so the request was answered as plain `de` and a concept with no German designation came back with the system's own display. A concept with nothing acceptable now carries no `display`, and the system's own reaches the answer as a designation.

- A request for the code system's own language reads the concept's display. `CodeSystem.concept.display` is already written in `CodeSystem.language`, and the provider looked for a designation first, so an `en` request against an `en` system could be answered with an alternate `en` designation ahead of the concept's own display.

- An unknown code is reported as `code-invalid` rather than `not-found`. The two issue types are different: `code-invalid` is "the code or system could not be understood, or it was not valid in the context", and `not-found` is "the reference provided was not found" (<https://hl7.org/fhir/R4B/valueset-issue-type.html>). A code system that resolved and a code it does not have is the first; the unresolvable system is still the second, with a 404.

- An expansion says which supplement it applied. A request may name one without a version, so the answer states the versioned canonical it resolved to as `used-supplement`, beside `used-codesystem` and `used-valueset`, rather than echoing the request's own `useSupplement`.

- An R6 `ConceptMap` element comment is read from the field R6 declares for it. R6 has `ConceptMap.group.element.comment` and R5 and earlier carry the same fact in an extension; only the extension was read, so an R6 document that stated the comment natively lost it on load and rendered without it.

- A parameter the operation does not declare is refused on `POST` as it is on `GET`. A `GET` was checked while its query was decoded and a `POST` carried a `Parameters` the codec read without consulting the operation, so the same parameter went through, was ignored, and the request answered 200 as though it had been honoured.

- Three contrast failures in the viewer, found by measuring every screen in both themes rather than by eye: the version switcher's label at 4.35:1, its selected version at 3.72:1, and the submit button in the dark theme at 3.74:1. The dark theme inverts the pairing rather than darkening the tint further, which would have fixed the text and broken the control's own boundary against the page.

- A concept map the deployment loaded reads at its own id and is found by a search. `GET ConceptMap` answered `total` 0 for a map `$translate` was resolving through, and `GET ConceptMap/{id}` answered nothing at all, while the capability statement declared `read` and `search-type` on the type. Every loaded map now answers both, under every version prefix, rendered in the reading version's own vocabulary: R4 and R4B carry `source[x]`, a `uri` `group.source` beside `sourceVersion`, `equivalence`, the `provided` unmapped mode, and an element that maps to nothing as a target with `equivalence = unmatched` and no code (<https://hl7.org/fhir/R4B/conceptmap.html>); R5 and R6 carry `sourceScope[x]`, a canonical `group.source`, `relationship`, `noMap` with `target` empty beside it as `cmd-4` requires, and `unmapped.otherMap` (<https://hl7.org/fhir/R5/conceptmap.html>).
- A resource in a `searchset` carries its own `Resource.id`, "the logical id of the resource, as used in the URL for the resource" (<https://hl7.org/fhir/R4B/resource.html#id>). A loaded `ValueSet` came back with a `fullUrl` and no id, so a client could see what a search returned and not read it; the read of a loaded value set carries the id too.
- `fhir-types` no longer forces `serde_json`'s `arbitrary_precision` feature on the crates that depend on it. Cargo unifies features across a build graph, so every dependent inherited a feature it cannot switch off, under which a JSON number deserializes as a private map and anything that buffers, an internally tagged enum among them, stops reading its own floats back. The feature was carrying a real requirement: FHIR says "Do not use an IEEE type floating point type, instead use something that works like a true decimal" (<https://hl7.org/fhir/R4B/datatypes.html#decimal>), so a decimal keeps the precision it was given. The codec now meets that requirement itself. It reads and writes FHIR JSON through its own document model, `fhir_types::codec::Value`, which holds a number in the text the document carried, so `1.10` and a decimal past `f64` precision round-trip unchanged on all four served versions through both write paths, and the crate turns on no `serde_json` feature that changes how anything else reads JSON. `Json::to_json`, `Json::from_json`, `Primitive::value_json`, and the XML conversion now speak `fhir_types::codec::Value` and `fhir_types::codec::Object` where they took `serde_json`'s. The bytes on the wire are unchanged, asserted over every resource of the four vendored FHIR packages and over the conformance suite.
- A `tx-resource` the request never resolves no longer refuses the whole request. One supplied resource that failed to deserialize took the request with it, so a `$expand` or `$validate-code` was answered 400 for a defect in a resource it does not touch. Cardinality is an aspect of validating a resource, which a server performs at its discretion, and an implementation "should be conservative in its sending behavior, and liberal in its receiving behavior" (<https://hl7.org/fhir/R4B/validation.html>). A supplied resource is now read leniently, so a required primitive that states no value reads as the value-less element it would be with only an extension, and a value set missing its `status` serves the request that names it. One the server still cannot use is recorded and answers only the request that resolves its `url`, with `issue.code` `invalid`, "content invalid against the specification or a profile" in the `IssueType` definition, where `structure` stays for a body the server cannot parse at all. A `compose.include.filter` with no value is such a resource: it answers `invalid` with the `vs-invalid` classification and `issue.expression` naming the filter. A `tx-resource` carrying a resource of a type the server never serves is still refused where it is stated.
- `ValueSet/$validate-code` no longer claims a code is outside a value set it cannot check. When the code's system is one the value set itself selects from and the server does not hold that system, membership is undetermined, so the answer now carries only the unknown-system issue beside `x-caused-by-unknown-system`. A system the request names that the value set does not select from is unchanged and still gets both issues.
- `inferSystem` names the systems that competed when more than one holds the code. The answer said only that the system could not be determined; it now lists the matching systems in the issue text, so a client can see why no single system was inferred.
- `$lookup` answers a `property` the code system does not define instead of dropping it. A client naming a property the server does not serve got a 200 with no signal, so a missing answer was indistinguishable from a concept with nothing to say. R5 and R6 bound the parameter to the codes the operation names itself plus "any property codes defined by this specification or by the CodeSystem", so a code outside that set is now a client error, and a property the code system's own specification defines that this server does not generate is `not-supported`. The two SNOMED CT normal form properties, `normalForm` and `normalFormTerse`, are the only ones in the second class today.

- A supplemented code system answers as the system it supplements. Naming a supplement (`useSupplement`, or the `valueset-supplement` extension a value set carries) layered a wrapper over the provider that answered its designations and properties and left most other questions to a default, so a supplemented SNOMED CT resolved no `?fhir_vs=` or `?fhir_cm=` URI and a supplemented BCP 13 refused a two-filter include that expands unsupplemented. `$expand` says naming a supplement behaves "the same way as if the supplements were included in the value set definition", and a supplement "extends an existing code system with additional designations and properties", so the wrapper now answers every other question with the supplemented system's own answer: the implicit value sets and concept maps and their metadata, the successors of an inactive concept, the inactive and abstract concept sets, filter evaluation and the bound a set of filters puts on a selection, subsumption the system decides itself, post-coordination, and the `CodeSystem` resource a resource-backed system serves.
- `$translate` over a SNOMED CT map reference set costs its answer rather than the release. The server built the whole `ConceptMap` of the named reference set on every request, one element and one store read per member, and then looked one code up in it, so the ICD-10 extended map of the International edition (137,678 members) answered in 28 seconds and every map answered in proportion to its size. A provider is now told which mappings a request needs, and the SNOMED provider reads only the reference set rows that carry them: the same request answers in 0.21 ms warm, and the three implicit maps of that edition, from 23,551 to 137,678 members, answer alike. Reading the whole map is unchanged and still available.

### Changed

- Every viewer screen is measured against WCAG 2.2 Level AA rather than checked by eye. The browser battery tabs through each screen in both themes and reports any control the keyboard never reached and any stop the browser drew no outline on, computes the contrast ratio of every text the browser painted, reads the tables, labels, ids and headings out of the DOM, and checks that a search, an expansion and a validation each announce themselves in a live region. The contrast is measured on the painted colour rather than on the token the source names, because Tailwind writes its palette in oklch and a browser resolves `color` in the space the author wrote.

- A SNOMED CT implicit `ConceptMap` states `group.element` in the edition's own concept order, the order `$expand` already uses, instead of the order the reference set file holds its rows in. No FHIR version orders `group.element`, and a selection of the elements has to read back in the same order as the whole map for a translation to answer identically either way. The matches a reverse `$translate` reports move with it; the set of matches does not change.
- A `$lookup` and a `$translate` answer in a fraction of the time, because a parameter costs its own value instead of the widest one the wire admits. A Rust enum is as large as its largest variant, and the generated `value[x]` enum admits `Dosage`, which carries a whole `Timing`: it was 4512 bytes, so every `ParametersParameter` cost 4736 whether it held a dosage schedule or the string `RxNorm`. An RxNorm `$lookup` writes 423 parameters and 945 nested parts, so it moved megabytes to say sixty kilobytes. The emitter now boxes every choice variant holding a complex type and leaves the primitives inline, which is what a terminology answer is nearly all of: the open type is 80 bytes and a parameter 304. The answer is also built once rather than copied into the wire tree. Measured over the local RxNorm and Netherlands artifacts, `$lookup` through the router went from about 630 to 232 microseconds on RxNorm and from about 300 to 139 on SNOMED CT. The response bytes are unchanged, asserted over every operation on all four served versions in both JSON and XML.
- A response is written from the typed resource. The generated FHIR types now carry their own `Serialize`, so `$lookup`, `$validate-code`, `$expand`, `$subsumes`, `$translate`, and every `OperationOutcome` write their answer straight into the response body instead of building a `serde_json` document first and writing that. The bytes are unchanged: the two paths are asserted identical over every resource of the four vendored FHIR packages and over generated `Parameters` documents, per version, so the primitive-plus-`_element` rule, the element order, and the absent-versus-null distinction are pinned where they were.

## [0.1.0] - 2026-09-06

### Added

- `FERROTERM_BASE_URL`: the URL clients reach the server at, stated per version as `CapabilityStatement.implementation.url` and in the terminology capabilities. A server behind a reverse proxy answers on an address it never sees, so the endpoint is configured rather than guessed from a socket or a forgeable header; a deployment that names none states no URL. `compose.yaml` gains a `proxied` profile: Caddy terminates TLS in front and the server publishes no host port.
- `GET /metrics`: the Prometheus exposition of the requests answered (a counter and a duration histogram per method, matched route, and status) and the code system versions loaded (one gauge each). The route is off the FHIR base path, so a scrape is never a terminology request, and the labels name the matched route rather than the URI, so the series count stays bounded whatever codes clients ask about.
- `X-Request-Id` on every response: the client's own value when it sent a usable one, a fresh UUID otherwise, and the same id in the request's log line. An id that is empty, over 128 characters, or not printable ASCII is replaced rather than echoed.
- `$batch-validate-code` on `ValueSet` and `CodeSystem`, on every served version: one `POST` carries the shared inputs once, a `validation` parameter per validation, each a `Parameters` of that validation's own inputs, and the answer repeats `validation` in the same order with that validation's `$validate-code` outputs. A validation states its own value for an input the request also states, and a validation the server cannot run answers an `OperationOutcome` in its own slot while the others still answer. No `OperationDefinition` declares the operation anywhere, so the contract is the terminology ecosystem suite's cases, whose `batch/batch-validate` now passes on `/r4b`, `/r4` and `/r5`.
- The fuzz targets under `fuzz/`: every parser an untrusted input reaches (ECL, the FHIR JSON and XML bodies, the SNOMED CT implicit `?fhir_vs=` and `?fhir_cm=` forms, and the RF2 file names, effective times, and identifiers) is fed arbitrary bytes by `cargo-fuzz`, weekly in CI and by hand for longer runs. The seeds are committed; a crash uploads the input that reproduces it.

### Fixed

- The terminology capabilities no longer claim SNOMED CT post-coordination. `CodeSystem.compositional` is "The code system defines a compositional (post-coordination) grammar" and `TerminologyCapabilities.codeSystem.version.compositional` is "If the compositional grammar defined by the code system is supported", two different statements that one flag answered, so a client was told it could send expressions and then got "no such concept" for every one. A provider now declares the grammar and this server's support for it separately: SNOMED CT defines the grammar and this server evaluates none, UCUM, BCP 13, BCP 47, and the ICD-11 linearizations parse the grammars they declare, and a `CodeSystem` resource that declares a grammar is served without one. A post-coordinated code is refused as a grammar this server does not evaluate (`not-supported`) rather than as a concept the edition lacks.
- A code system the deployment loaded reads at its own id and is found by a search. `GET CodeSystem/{id}` on the id the server prints at startup was a `404` and `GET CodeSystem?url=…` answered `total` 0, while the capability statement declared `read` and `search-type` and the configuration page documented the id, so a documented instance path addressed nothing. Every loaded version now answers both, under every version prefix: a system held behind an index answers its definition with `content = not-present`, and one loaded as a `CodeSystem` resource answers with the `content` it declared. A supplement stays applied to the system it supplements and is not served as an instance.
- `CodeSystem/{id}/$lookup` answers under `/r5` and `/r6`, whose `OperationDefinition` declares the operation at the instance level. The engine refused the instance level on every version; the level is now the served version's own declaration, so `/r4` and `/r4b`, which declare the type level only, still offer no such route.
- A SNOMED CT implicit value set or concept map based on a non-default edition answers when several editions are loaded. The FHIR SNOMED CT page lets any edition version stand as the base URL, and the edition in the base decides the membership, but the server only asked the default version of the system, so every `?fhir_vs` form and `?fhir_cm=` on another loaded edition was refused as a malformed URI. The registry now asks every loaded version, default first, so the bare `http://snomed.info/sct` base still answers from the default edition. A base naming an edition version no loaded version serves is a `not-found` on that version rather than `vs-invalid`.
- `$translate` over a SNOMED CT map reference set returns a target `Coding` carrying both a `system` and a `code`. R4B needs `ConceptMap.group.target`, the "absolute URI that identifies the target system that the concepts will be mapped to", unless the target value set names a single system or every target equivalence is `unmatched`, and an implicit map states neither. RF2 records the scheme nowhere on a member row, so the reference set names it: the ICD-10 extended map (`447562003`), the ICD-9-CM equivalence map (`447563008`), the ICD-O map (`446608001`), the CTV3 map (`900000000000497000`), and the US Edition's ICD-10-CM map (`6011000124106`) each state the URI FHIR publishes for their scheme, on the served `ConceptMap` and on every match. A map reference set outside that table answers `not-supported` instead of a code with no system. An association reference set names SNOMED CT on both sides, whichever reference set it is.
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

[Unreleased]: https://github.com/rubentalstra/FerroTERM/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/rubentalstra/FerroTERM/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/rubentalstra/FerroTERM/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.1.1
[0.1.0]: https://github.com/rubentalstra/FerroTERM/releases/tag/v0.1.0
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

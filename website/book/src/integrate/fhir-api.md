# The FHIR terminology API

FerroTERM speaks the HL7 FHIR terminology API. If your client already talks to
a FHIR terminology server, it talks to FerroTERM. It speaks FHIR JSON and
FHIR XML: `_format=xml` in the query or `Accept: application/fhir+xml`
selects XML for a response, `Content-Type` names the format of a request
body, and JSON is the default. The base is `/r4b` for R4B,
`/r4` for R4, `/r5` for R5, and `/r6` for the R6 ballot (see
[FHIR versions](fhir-versions.md)); the
[Worked examples](examples.md) page shows requests and the responses a running
server gave.

<!-- toc -->

## The operations

| Operation | Answers | How |
|---|---|---|
| `CodeSystem/$lookup` | a concept's display, designations, and properties | a point read in the store; `property` narrows, `displayLanguage` picks the language |
| `CodeSystem/$validate-code` | whether the code exists and the display is right | the store; the right display comes back when the given one is wrong; inactive codes validate with a message |
| `CodeSystem/$subsumes` | `subsumes`, `subsumed-by`, `equivalent`, or `not-subsumed` | a bitmap membership test in the closure |
| `ValueSet/$expand` | the members of a value set, paged | the compose layer over the providers' filters and bitmaps |
| `ValueSet/$validate-code` | whether a code is a member, and its display | membership without enumerating the set |
| `ConceptMap/$translate` | the targets a code maps to, with the relationship | loaded, inline, or request-scoped maps |
| `$versions` | the FHIR version of the base (`4.3`) | |
| `$cache-control` | a cache of request-scoped resources | below |
| `metadata`, `metadata?mode=terminology` | the `CapabilityStatement` and the `TerminologyCapabilities` listing every served code system with its filters and properties | |

Every operation accepts `GET` with query parameters and `POST` with a
`Parameters` resource, at the type level (`ValueSet/$expand?url=…`) and at
the instance level (`CodeSystem/{id}/$validate-code`). `$expand` returns the
expanded `ValueSet`; the others return `Parameters`. Every failure is an
`OperationOutcome` whose issue carries `severity`, `code`, and `details.text`,
never a bare 500. A terminology failure adds a `details.coding` from
`http://hl7.org/fhir/tools/CodeSystem/tx-issue-type` (`invalid-code`,
`not-found`, `vs-invalid`, `too-costly`, and the rest); a refusal at the wire
layer, such as an unreadable id or a failed precondition, carries the status
and the text without one. The parameter set of each operation is exactly what the R4B
`OperationDefinition` declares; a parameter another version defines is refused.

## Value sets

A value set reaches `$expand` and `ValueSet/$validate-code` in five ways:

- **Loaded**: a `ValueSet` JSON under `FERROTERM_CODESYSTEMS`, named by `url`
  (and `version`; without one the greatest version answers). Loaded value sets
  are readable at `GET ValueSet/{id}` and `GET ValueSet?url=…&version=…`.
- **Persisted**: a `ValueSet` written through the REST API (see below).
- **Inline**: the `valueSet` parameter of a `POST`.
- **Request-scoped**: `tx-resource` parameters, see below.
- **Implicit**: a URL a code system defines, answered by its provider without
  a resource: LOINC's `http://loinc.org/vs`, `/vs/[answer list]`, and
  `/vs/[part]`; UCUM's `/vs` and `/vs/[unit]`; RxNorm's `/vs`; ICD-11's
  `<entity>/postcoordinationScale/<axis>`; SNOMED CT's `?fhir_vs`,
  `?fhir_vs=isa/[sctid]`, `?fhir_vs=refset`, and `?fhir_vs=refset/[sctid]`,
  on the system, edition, or version URI ([Value sets and ECL](ecl-value-sets.md)).

A `compose` may include whole systems, enumerated concepts, and filters. The
filters a system answers are the ones its provider declares in
`TerminologyCapabilities`, and they differ by system, so read the declaration
rather than assuming a common set. SNOMED CT declares `concept` with `is-a`,
`descendent-of`, and `in`, then `constraint` for an expression constraint and
`expressions` for post-coordination. The others declare their own: LOINC's
`parent` and `ancestor` and its table fields, RxNorm's `STY`, `SAB`, `TTY`,
`REL`, and `RELA`, UCUM's `canonical` and `property`, ICD-10's note kinds. `$expand`
honours `count` and `offset` (at most 1,000 members without `count`), `filter`
(word-prefix text search over designations), `activeOnly`, `excludeNested`,
`includeDesignations`, `displayLanguage`, `system-version`,
`check-system-version`, `force-system-version`, and `exclude-system`, and
echoes every effective parameter plus one `used-codesystem` per system version
the expansion drew on.

An expansion nests where the compose is the system's own hierarchy: one include
of one system, taken whole or narrowed by `is-a` or `descendent-of`, with
nothing enumerated and nothing excluded. Each root then carries its children
under `contains`
(<https://hl7.org/fhir/R4B/valueset-definitions.html#ValueSet.expansion.contains>),
`total` counts every member rather than the roots, and a page is the slice of
the pre-order walk. `excludeNested=true` flattens it, and a text `filter` over a
whole system is flat already, because its matches have no root to hang from.

## Request-scoped resources

A client can bring its own `CodeSystem`, `ValueSet`, and `ConceptMap`
resources for one request: each `tx-resource` parameter of a `POST` carries
one, and the operation sees them layered over the loaded resources (a resource
with the same `url` and `version` as a loaded one shadows it for that request).
A supplement in a `tx-resource` applies to the system it names. Any other
resource type is refused with `not-supported`. This is what the HL7 terminology
ecosystem runner sends; `tx-resource` is a declared parameter in R6 and an
ecosystem extension on R4B
(<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/requirements.html>).

To avoid resending the same resources, start a cache:

```text
POST [base]/$cache-control?mode=start   (Parameters of tx-resource)
→ Parameters { cache-id }
```

Then name it with the `X-Cache-Id` header on later requests; their own
`tx-resource`s stack on top. `POST [base]/$cache-control?mode=end` with the
header (or a `cache-id` parameter) releases it. A cache unused for 30 minutes
expires, and an unknown id answers `404`.

## Code system identity and versions

`system` is the code system's canonical URI (`http://snomed.info/sct`,
`http://loinc.org`, `http://hl7.org/fhir/sid/icd-10-cm`, …). Each loaded
version is one `CodeSystem` instance; without a `version` the server resolves
its default and echoes the version it used in every response. For SNOMED CT
the version is the edition and version URI of the SNOMED CT URI standard
(`http://snomed.info/sct/11000146104/version/20260630`), never a bare date.

## Languages

`displayLanguage` and the `Accept-Language` header both select the display
and, on `$expand`, the displays of the members; the parameter wins when both
are given. Either carries a language range list with quality values
(`en, en-AU; q=0.4`, `de,*`), and the first range the code system carries, by
quality then by position, is the display language; `*` is the system's own.
A language the code system does not carry falls back to the system's own
(English for LOINC and ICD-11, the classification's language for a ClaML
document, the default language for SNOMED CT), and the response states what
it used. Designations carry their BCP 47 language and their use.

## Persisted resources

A deployment that names a database in `FERROTERM_RESOURCES` accepts
`CodeSystem`, `ValueSet`, and `ConceptMap` resources over the REST API and
keeps them across restarts. The interactions are the ones the FHIR RESTful API
defines (<https://hl7.org/fhir/R4B/http.html>), under every version prefix:

| Request | Answer |
|---|---|
| `POST {type}` | `201 Created` with `Location`, `ETag`, and `Last-Modified`; the server assigns the id |
| `PUT {type}/{id}` | `200 OK` when the id existed, `201 Created` when it is new |
| `GET {type}/{id}` | `200 OK` with `ETag` and `Last-Modified`, `404` for an unknown id, `410 Gone` for a deleted one |
| `GET {type}/{id}/_history/{versionId}` | `200 OK` with that version of the resource |
| `GET {type}?url=…&version=…` | a `searchset` `Bundle` |
| `DELETE {type}/{id}` | `204 No Content`, and deleting again has no effect and is not an error |

`meta.versionId` starts at `1` and rises with every write; the `ETag` is its
weak form (`W/"2"`). Send `If-Match` with that value on a `PUT` or a `DELETE`
to make the write conditional; a value that names another version is a
`412 Precondition Failed`.

A resource is stored as the JSON it arrived as, with the FHIR version of the
endpoint that received it, and a read renders it through the reading version's
own codec. Reading a resource on a version that does not define an element it
carries is a `422`. Every operation sees a persisted resource exactly as it
sees one loaded from `FERROTERM_CODESYSTEMS`, on every served version.

A deployment that names no database refuses every write with a `422` and
declares no write interaction in its capability statement.

## SNOMED CT implicit concept maps

`ConceptMap/$translate` answers `url=http://snomed.info/sct?fhir_cm=[sctid]`,
where the SCTID names a reference set of the loaded edition. The edition or
edition-version URI may take the place of the bare system URI; the map states
the served edition version either way
(<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Concept Maps").

Two kinds of reference set answer. An association reference set maps a SNOMED
concept to another SNOMED concept, and the FHIR SNOMED CT page fixes the
relationship each one asserts:

| Reference set | SCTID | Relationship |
|---|---|---|
| POSSIBLY EQUIVALENT TO | `900000000000523009` | `inexact` |
| REPLACED BY | `900000000000526001` | `equivalent` |
| SAME AS | `900000000000527005` | `equal` |
| ALTERNATIVE | `900000000000530003` | `inexact` |

R5 and R6 have no `equal` or `inexact` code, so those become `equivalent` and
`related-to` on those versions, as the generated relationship vocabulary maps
them.

A map reference set maps a SNOMED concept to a code of another system through
`mapTarget`. No RF2 file records which system that code belongs to, so the
reference set itself says which scheme it maps to, and the group names that
system:

| Reference set | SCTID | Target system |
|---|---|---|
| ICD-10 extended map | `447562003` | `http://hl7.org/fhir/sid/icd-10` |
| ICD-9-CM equivalence complex map | `447563008` | `http://hl7.org/fhir/sid/icd-9-cm` |
| ICD-O simple map | `446608001` | `http://terminology.hl7.org/CodeSystem/icd-o-3` |
| CTV3 simple map | `900000000000497000` | `http://terminology.hl7.org/CodeSystem/read-Codes` |
| ICD-10-CM complex map (US Edition) | `6011000124106` | `http://hl7.org/fhir/sid/icd-10-cm` |

R4B needs the group to name a target system whenever the targets are real
codes: it may be left out only when the target value set names a single system,
or when every target equivalence is `unmatched`
(<https://hl7.org/fhir/R4B/conceptmap-definitions.html#ConceptMap.group.target>).
A map reference set outside the table, such as a national extension's own map,
is answered with `not-supported` rather than a target `Coding` that carries a
code and no system. RF2 records no version of the target scheme, so the group
names the system alone.

The complex and extended map columns (`mapGroup`, `mapPriority`, `mapRule`,
`mapAdvice`, `correlationId`, `mapCategoryId`) travel as `product` parts of the
match. No specification says where those columns belong in a `$translate`
result; this placement is FerroTERM's own design.

A `$translate` that names no `url` and finds no stored map falls back to the
historical associations of an inactive concept: the `SAME AS` and `REPLACED BY`
reference sets name what stands in its place, and each match names the implicit
map it came from. No specification asks for this fallback; it is FerroTERM's
own design. An active concept has no successors.

A `?fhir_cm=` naming a reference set the edition does not hold, or one that
maps nothing, is a `not-found`; a base that is not the served edition, or an
SCTID that does not parse, is a `400`.

## Batch

`POST [base]` with a `Bundle` of type `batch` runs every entry and answers a
`batch-response` with one entry per request, in the order they were sent
(<https://hl7.org/fhir/R4B/http.html#transaction>). The entries are
independent: one that fails answers the `OperationOutcome` that the same
request would have answered on its own, the rest still answer, and the batch
itself answers `200`.

Each entry names its request the way it would have been sent on its own:

```json
{"resourceType": "Bundle", "type": "batch", "entry": [
  {"request": {"method": "GET", "url": "CodeSystem/$lookup?system=http://loinc.org&code=1963-8"}},
  {"request": {"method": "POST", "url": "ValueSet/$validate-code"},
   "resource": {"resourceType": "Parameters", "parameter": [
     {"name": "url", "valueUri": "http://loinc.org/vs"},
     {"name": "coding", "valueCoding": {"system": "http://loinc.org", "code": "1963-8"}}
   ]}}
]}
```

A `GET` entry carries the operation's inputs in the query of `request.url`; a
`POST` entry carries them in a `Parameters` resource on the entry. Every
terminology operation this server answers is reachable, at the type level and
at the instance level, and `entry.fullUrl` comes back on the response entry so
a client can pair the two. `response.status` is the code and its reason, such
as `200 OK` or `404 Not Found`.

A `transaction` Bundle is refused with `not-supported`: a transaction succeeds
or fails as one unit across its entries, and this server does not process one.

A failed entry carries its `OperationOutcome` as the entry's `resource`, the
body the same request would have answered alone. `entry.response.outcome` is
reserved by the specification for hints and warnings and is "not used for error
responses in batch/transaction", and no version names another slot for the
error, so this placement is FerroTERM's reading of an ambiguous rule.

## Closure tables

`POST [base]/$closure` maintains a named transitive closure table for a client
(<https://hl7.org/fhir/R4B/terminology-service.html>, "Maintaining a Closure
Table"). The client registers concepts as it meets them and the server answers
with the subsumption relationships that hold between them, as a `ConceptMap`
delta. Any code system with subsumption maintains a closure, not only SNOMED CT.

| Request | Answer |
|---|---|
| `name` alone | the table, created or emptied, as an empty `ConceptMap` at version `0` |
| `name` and one or more `concept` | a `ConceptMap` of the relationships the client did not have, at a new version |
| `name` and `version` | everything the server sent after that version, at the server's latest version |

A `version` of `0` resynchronises the whole table. Pass a `concept` or a
`version`, never both. The version is the server's own value: treat it as
opaque and hand it back unchanged.

An entry's `equivalence` is read from target to source, so `subsumes` means the
target subsumes the source. R4 and R4B use `equal`, `subsumes`, and
`specializes`; R5 states the same relationships as `equivalent`,
`source-is-narrower-than-target`, and `source-is-broader-than-target`. A concept
is never related to itself, and a client assumes that relationship. Two concepts
that subsume neither way get no entry.

Naming a table the server has not been asked to create is a `404`; an add never
creates one. Registering a concept whose code system changed under the table
answers `422` with `closure "[name]" must be reinitialized`, and the client's
move is to initialise and replay its codes.

The R6 ballot ships no `ConceptMap-closure` definition, so `/r6` offers no
`$closure` and its capability statement declares none. The tables live in the
database `FERROTERM_RESOURCES` names, so they outlive a restart.

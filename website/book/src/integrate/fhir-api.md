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
`OperationOutcome` whose issue carries `severity`, `code`, `details.text`, and
a `details.coding` from `http://hl7.org/fhir/tools/CodeSystem/tx-issue-type`
(`invalid-code`, `not-found`, `vs-invalid`, `too-costly`, …), never a bare
500. The parameter set of each operation is exactly what the R4B
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
`TerminologyCapabilities`: the generic `concept is-a`, `descendent-of`,
`is-not-a`, `generalizes`, `in`, `not-in`, `regex`, and `exists` over any
system with a hierarchy, plus each system's own (LOINC's `parent` and
`ancestor` and its table fields, RxNorm's `STY`, `SAB`, `TTY`, `REL`, and
`RELA`, UCUM's `canonical` and `property`, ICD-10's note kinds). `$expand`
honours `count` and `offset` (at most 1,000 members without `count`), `filter`
(word-prefix text search over designations), `activeOnly`,
`includeDesignations`, `displayLanguage`, `system-version`,
`check-system-version`, `force-system-version`, and `exclude-system`, and
echoes every effective parameter plus one `used-codesystem` per system version
the expansion drew on. Expansions are flat today; nested `contains` is the
v0.0.11 milestone.

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

# FerroTERM

[![CI](https://github.com/rubentalstra/FerroTERM/actions/workflows/ci.yml/badge.svg)](https://github.com/rubentalstra/FerroTERM/actions/workflows/ci.yml)
[![CodeQL](https://github.com/rubentalstra/FerroTERM/actions/workflows/codeql.yml/badge.svg)](https://github.com/rubentalstra/FerroTERM/actions/workflows/codeql.yml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/rubentalstra/FerroTERM/badge)](https://scorecard.dev/viewer/?uri=github.com/rubentalstra/FerroTERM)
[![Quality Gate Status](https://sonarcloud.io/api/project_badges/measure?project=rubentalstra_FerroTERM&metric=alert_status)](https://sonarcloud.io/summary/overall?id=rubentalstra_FerroTERM)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

A pure-Rust FHIR terminology server for SNOMED CT, LOINC, ICD-10, ICD-11,
RxNorm, UCUM, and any FHIR `CodeSystem`, served from one static binary over a
memory-mapped index. No JVM, no Elasticsearch, no database to run.

> **FerroTERM** is the official name: Ferro for the Rust family it shares with
> [FerroEHR](https://github.com/rubentalstra/FerroEHR), TERM for terminology.
> The site is <https://ferroterm.eu>; the book is <https://ferroterm.eu/docs/>.

## Five minutes to a running server

The image serves UCUM, BCP 47, BCP 13, and ISO 3166-1 with no configuration,
so the first call needs nothing beyond Docker:

```console
$ docker run --rm -p 8080:8080 ghcr.io/rubentalstra/ferroterm:0.0.8
$ curl 'http://localhost:8080/r4b/CodeSystem/$lookup?system=http://unitsofmeasure.org&code=mg/dL'
```

```json
{
  "resourceType": "Parameters",
  "parameter": [
    { "name": "name", "valueString": "Unified Code for Units of Measure (UCUM)" },
    { "name": "version", "valueString": "2.2" },
    { "name": "display", "valueString": "mg/dL" },
    { "name": "property", "part": [
      { "name": "code", "valueCode": "canonical" },
      { "name": "value", "valueCode": "m-3.g" } ] }
  ]
}
```

A clinical code system is a release you hold a licence for, built once into an
index by `ferroterm-build` (in the same image and every release tarball) and
served read-only. With the release `compose.yaml` and a SNOMED CT release zip:

```console
$ curl -LO https://github.com/rubentalstra/FerroTERM/releases/latest/download/compose.yaml
$ FERROTERM_RF2=/path/to/SnomedCT_Release.zip docker compose run --rm build
$ docker compose up
$ curl 'http://localhost:8080/r4b/CodeSystem/$lookup?system=http://snomed.info/sct&code=404684003&displayLanguage=nl'
```

The Dutch edition (548,949 concepts, active and inactive) builds in 49 s into a
591 MB index; the server opens it in half a second and answers a `$lookup` in
about 2.5 ms end to end on a laptop, `curl` included. The book's
[Install and run](https://ferroterm.eu/docs/operate/install.html) and
[Loading code systems](https://ferroterm.eu/docs/operate/loading-snomed.html)
pages cover every system and the binary distribution.

## What it serves

| Code system | Source you provide | Notes |
|---|---|---|
| SNOMED CT (any edition) | the RF2 release zip | preferred terms per language reference set, inactive concepts marked, the edition and version URI in `version` |
| LOINC | `Loinc_x.yy.zip` | terms, parts, answer lists, linguistic variants; the FHIR LOINC filters and `/vs/` value sets |
| ICD-10 (WHO, national translations), ICPC-2 | a ClaML document | `classified-with` tree, modifiers expanded, notes as properties |
| ICD-10-CM | the CMS tabular and order files | codes with the period, seventh-character codes, the `valid` flag |
| ICD-11 (MMS, ICF, the Foundation) | the WHO ICD-API local deployment | codes, entity URIs, and postcoordination expressions validated against the axes |
| RxNorm | the RRF release, or the licence-free prescribable subset | the FHIR `STY`, `SAB`, `TTY`, `REL`, and `RELA` filters over typed edges |
| ATC/DDD | the WHO index as CSV, or the G-Standaard `BST801T` file | the five levels as a tree, DDDs as properties |
| DHD thesauri | the Uitleverformaat 5.0 delivery zip | a flat table with SNOMED CT, ICD-10, DBC, and ZA links as properties and concept maps |
| G-Standaard | the monthly release directory (`BSTnnnT` files) | GPK, PRK, HPK, and article systems, the ladder as properties |
| Any FHIR `CodeSystem`, `ValueSet`, `ConceptMap` | a FHIR package or a directory of JSON | HL7 Terminology's 900+ systems load this way; supplements apply to loaded systems |
| UCUM, BCP 47, BCP 13, ISO 3166-1 | nothing | grammar and registry systems vendored into the binary |

Every system reaches the operations through one provider seam
([`docs/terminologies.md`](docs/terminologies.md) records each system's FHIR
page, licence, and what the provider answers), so nothing in an operation is a
special case for one system.

## The API

FHIR R4 under `/r4`, R4B under `/r4b`, and R5 under `/r5`, each in its own
version's shapes:
`CodeSystem/$lookup`, `CodeSystem/$validate-code`,
`CodeSystem/$subsumes`, `ValueSet/$expand` (paging, `filter`, version
pins, inline and request-scoped value sets), `ValueSet/$validate-code`,
`ConceptMap/$translate`, `GET ValueSet/{id}` and `GET ValueSet?url=`,
`$versions`, `$cache-control`, `metadata`, and
`metadata?mode=terminology`. Every failure is an `OperationOutcome` with a
`tx-issue-type` coding.

Conformance is measured, not asserted: CI runs the HL7 terminology ecosystem
suite against every pull request and holds a committed pass list (120 of the
670 general cases on R4B; the rest are R5-shape cases and features on the
roadmap, listed on the tracker).

## What is next

The tracker's milestones are the roadmap:

- **v0.0.9**: R4, R5, and R6 endpoints from the generated per-version modules,
  and the XML wire format.
- **v0.0.10**: hierarchical `$expand`, persisted client resources, `Bundle`
  batch, `$closure`, and the SNOMED implicit concept maps.

## How it is built

The design, with its citations, is [`docs/architecture.md`](docs/architecture.md).
The short form:

- **Offline once, online from precomputed structures.** `ferroterm-build`
  turns a release into a `redb` store (concepts, designations, properties),
  a CSR is-a adjacency with roaring transitive-closure bitmaps
  (`hierarchy.bin`), and an `fst` word index (`text.bin`). The server
  memory-maps them read-only; subsumption is a bitmap test and a
  descendant set is a bitmap.
- **FHIR is generated, never hand-written.** `crates/fhir-types` is
  emitted from the pinned HL7 packages (R4 4.0.1, R4B 4.3.0, R5 5.0.0, R6
  ballot 5, HL7 Terminology) so each version's operation surface is right by
  construction; a drift check regenerates it in CI.
- **The engine is code-system-neutral.** Providers own the semantics
  (`crates/fhir-terminology`); the operations talk to the seam.
- **Supply chain.** Releases are built in a reusable workflow to SLSA Build
  Level 3: signed provenance, CycloneDX SBOMs, `cargo auditable` binaries, a
  distroless image for `linux/amd64` and `linux/arm64`, verifiable with
  `gh attestation verify`.

## Licensing

The software is Apache 2.0 ([`LICENSE`](LICENSE), [`NOTICE`](NOTICE)). The code
systems are not: SNOMED CT is licensed by SNOMED
International, LOINC by Regenstrief, ICD by WHO, RxNorm by NLM, and the
repository ships none of their content. You bring the release you are licensed
for; UCUM and the IANA and Unicode registries are vendored under their own
licences, recorded beside the data.

## Contributing

[`CONTRIBUTING.md`](CONTRIBUTING.md) has the rules; the open issues are the
worklist. Every change ships with tests, and conformance-facing behaviour cites
the FHIR or SNOMED specification it implements.

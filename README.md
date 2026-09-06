# <img src="https://raw.githubusercontent.com/rubentalstra/FerroTERM/main/assets/brand/ferroterm-lockup-auto.svg" alt="FerroTERM" width="284" height="64">


[![CI](https://github.com/rubentalstra/FerroTERM/actions/workflows/ci.yml/badge.svg)](https://github.com/rubentalstra/FerroTERM/actions/workflows/ci.yml)
[![CodeQL](https://github.com/rubentalstra/FerroTERM/actions/workflows/codeql.yml/badge.svg)](https://github.com/rubentalstra/FerroTERM/actions/workflows/codeql.yml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/rubentalstra/FerroTERM/badge)](https://scorecard.dev/viewer/?uri=github.com/rubentalstra/FerroTERM)
[![Quality Gate Status](https://sonarcloud.io/api/project_badges/measure?project=rubentalstra_FerroTERM&metric=alert_status)](https://sonarcloud.io/summary/overall?id=rubentalstra_FerroTERM)
[![Coverage](https://sonarcloud.io/api/project_badges/measure?project=rubentalstra_FerroTERM&metric=coverage)](https://sonarcloud.io/summary/new_code?id=rubentalstra_FerroTERM)
[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](LICENSE)
[![GitHub release (latest SemVer)](https://img.shields.io/github/v/release/rubentalstra/FerroTERM?sort=semver)](https://github.com/rubentalstra/FerroTERM/releases/latest)

[![tx-ecosystem R4](https://img.shields.io/endpoint?url=https%3A%2F%2Fferroterm.eu%2Fconformance%2Fr4.json)](https://ferroterm.eu/docs/evaluate/conformance.html)
[![tx-ecosystem R4B](https://img.shields.io/endpoint?url=https%3A%2F%2Fferroterm.eu%2Fconformance%2Fr4b.json)](https://ferroterm.eu/docs/evaluate/conformance.html)
[![tx-ecosystem R5](https://img.shields.io/endpoint?url=https%3A%2F%2Fferroterm.eu%2Fconformance%2Fr5.json)](https://ferroterm.eu/docs/evaluate/conformance.html)

A pure-Rust FHIR terminology server for SNOMED CT, LOINC, ICD-10, ICD-11,
RxNorm, UCUM, and any FHIR `CodeSystem`, served from one binary over an index
built once per release and read at startup. No JVM, no Elasticsearch, no
database to run.

## Five minutes to a running server

The image serves UCUM, BCP 47, BCP 13, and ISO 3166-1 with no configuration,
so the first call needs nothing beyond Docker:

```console
$ docker run --rm -p 8080:8080 ghcr.io/rubentalstra/ferroterm:0.1.0
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

What each code system costs to build and serve, and how fast each operation
answers, is in the [speed and footprint table](#speed-and-footprint) below,
rendered from the benchmark records. The book's
[Install and run](https://ferroterm.eu/docs/operate/install.html) and
[Loading code systems](https://ferroterm.eu/docs/operate/loading-snomed.html)
pages cover every system and the binary distribution.

## What it serves

<!-- code-systems:begin -->
SNOMED CT, LOINC, UCUM, BCP 47, BCP 13, ISO 3166-1, ICD-10 (WHO), ICD-10-NL,
ICD-10-CM, ICD-11 MMS, ICD-11 ICF, ICD-11 Foundation, ATC/DDD, ICPC-2, RxNorm,
the DHD Diagnosethesaurus and Verrichtingenthesaurus, the G-Standaard, the
Nederlandse Labcodeset, the NHG ICPC-1 to SNOMED CT map, and any FHIR
`CodeSystem`, `ValueSet`, and `ConceptMap` resources (HL7 Terminology's 900+
systems load this way).
<!-- code-systems:end -->

The [code systems page](https://ferroterm.eu/docs/evaluate/code-systems.html)
of the book is the one list: per system the canonical URI, the versions and
editions handled, the build command, and the licence position. You bring the
release you are licensed for; UCUM and the registries are vendored into the
binary and need nothing. Every system reaches the operations through one
provider seam, so nothing in an operation is a special case for one system.

## Speed and footprint

Measured by the benchmark harness on one machine, one record per
code system, warm p50 per operation; nothing here is typed by hand, and CI fails
when the table drifts from the records under `bench/records/`.

<!-- bench-table:begin -->
| Code system | Release | Concepts | Build | Peak build memory | Index on disk | Resident | `$lookup` | `$validate-code` | `$subsumes` | `$expand` (small) | `$expand` (large) | Search | Snowstorm |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| [ICD-10-CM](bench/records/2026-09-06-apple-m2/icd-10-cm-2026-09-06T05-37-22-012999Z.json) | 2026 | 98,827 | 1.54 s | 345 MB | 40 MB | 62 MB | 89 µs | 58 µs | 54 µs | n/a | n/a | n/a | not run |
| [ICD-10-NL](bench/records/2026-09-06-apple-m2/icd-10-nl-2026-09-06T05-37-23-26765Z.json) | 2021 | 42,769 | 1.02 s | 231 MB | 20 MB | 36 MB | 160 µs | 88 µs | 45 µs | n/a | n/a | n/a | not run |
| [ICD-11 MMS](bench/records/2026-09-06-apple-m2/icd-11-mms-2026-09-06T05-37-28-643697Z.json) | 2026-01 | 37,211 | n/a | n/a | 34 MB | 74 MB | 198 µs | 60 µs | n/a | 71 µs | 68 µs | 81 µs | not run |
| [LOINC](bench/records/2026-09-06-apple-m2/loinc-2026-09-06T05-37-20-302518Z.json) | 2.83 | 257,266 | 10.15 s | 2.14 GB | 262 MB | 170 MB | 186 µs | 59 µs | n/a | 60 µs | 8.28 ms | 241 µs | not run |
| [RxNorm (prescribable subset)](bench/records/2026-09-06-apple-m2/rxnorm-prescribable-subset-2026-09-06T05-37-28-318855Z.json) | 09082026 | 81,468 | 4.6 s | 640 MB | 72 MB | 119 MB | 987 µs | 117 µs | n/a | n/a | n/a | n/a | not run |
| [SNOMED CT (International edition)](bench/records/2026-09-06-apple-m2/snomed-ct-international-edition-2026-09-06T05-37-08-240287Z.json) | 20260901 | 535,502 | 22.05 s | 2.85 GB | 626 MB | 702 MB | 517 µs | 94 µs | 64 µs | 178 µs | 1.99 ms | 307 µs | not run |
| [SNOMED CT (Netherlands edition)](bench/records/2026-09-06-apple-m2/snomed-ct-netherlands-edition-2026-09-06T05-36-45-006081Z.json) | 20260630 | 548,949 | 34.83 s | 3.62 GB | 864 MB | 889 MB | 543 µs | 80 µs | 78 µs | 254 µs | 2.78 ms | 801 µs | not run |

Warm p50 over 200 HTTP round trips on one machine (Apple M2, 17.18 GB, macos/aarch64), FerroTERM 0.1.0 serving FHIR R4B, taken 2026-09-06. The records are under `bench/records/`; the [benchmarks page](https://ferroterm.eu/benchmarks.html) has the method, the cold and tail latencies, and how to reproduce a record.
<!-- bench-table:end -->

## The API

FHIR R4 under `/r4`, R4B under `/r4b`, R5 under `/r5`, and the R6 ballot under
`/r6`, each in its own version's shapes:
`CodeSystem/$lookup`, `CodeSystem/$validate-code`,
`CodeSystem/$subsumes`, `ValueSet/$expand` (paging, `filter`, version
pins, nested `contains`, inline and request-scoped value sets),
`ValueSet/$validate-code`,
`ConceptMap/$translate` (loaded, inline, and the SNOMED implicit concept maps),
`$closure` (on `/r4`, `/r4b`, and `/r5`; the R6 ballot defines none),
`POST [base]` with a `batch` `Bundle`, `GET ValueSet/{id}` and
`GET ValueSet?url=`, `$versions`, `$cache-control`, `metadata`, and
`metadata?mode=terminology`. A deployment that names a database in
`FERROTERM_RESOURCES` also serves `POST`, `PUT`, `GET`, `DELETE`, `_history`
reads, and `?url=` search of `CodeSystem`, `ValueSet`, and `ConceptMap`, with
`ETag` and `If-Match`. Every failure is an `OperationOutcome`, never a bare
500, and a terminology failure carries a `tx-issue-type` coding.

Conformance is measured. CI runs the HL7 terminology ecosystem
suite against every pull request and holds a committed pass list per served
version (523 of the 670 general cases on R5, 518 on R4, 517 on R4B; the rest are
features on the roadmap and fixture artefacts, listed by cluster on the
tracker). Every route answers FHIR JSON or FHIR XML, by `_format` or `Accept`.

## What is next

The tracker's milestones are the roadmap:

- **v0.1.0**: every public claim on the README, the site, and the book checked
  against the code and the recorded evidence before the cut.
- **v0.1.1**: the open HL7 terminology ecosystem suite cases, and the read and
  build paths measured against the latency and ingest bars.
- **v0.2.0**: the differential check against the Nictiz Nationale
  Terminologieserver for the Dutch variants; the Snowstorm differential harness
  runs today.

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
  Level 3: signed provenance, an SBOM per artifact (CycloneDX for the
  binaries, SPDX for the image), `cargo auditable` binaries, a distroless
  image for `linux/amd64` and `linux/arm64`, verifiable with
  `gh attestation verify`.

## Licensing

The software is source-available under the Business Source License 1.1
([`LICENSE`](LICENSE), [`NOTICE`](NOTICE)), with no open-core tier: the
engine, the server, and the tools are in this repository under the one
licence, and nothing is held back to be sold back to you.

The licence lets you read, build, modify, and redistribute the source without
a fee and without asking anyone, and it covers every non-production use:
development, testing, evaluation, and prototyping. Production use is free for
Non-Commercial Purposes, which the licence defines as personal use, academic or
scientific research, teaching, and use by a non-profit organisation or public
body that is not in the course of a business, does not deliver a service for
payment, and is not for commercial advantage. Any other production use needs a
commercial licence from the Licensor: a hospital, clinic, or care provider
running FerroTERM for its patients needs one, and so does a vendor, integrator,
or any company running it in production. Offering FerroTERM, or a work derived
from it, to third parties as a hosted, managed, or embedded terminology
service, and selling, sublicensing, or otherwise distributing it for a fee on
its own or inside another product, need a commercial licence in every case.
Each version becomes Apache License 2.0 four years after that version is
published. Two crates are outside all of this: `fhir-types` (the FHIR types and
operation contracts generated from HL7's own packages) and `rf2` (the SNOMED CT
release file reader) are Apache 2.0 on crates.io, so any Rust project can use
them without a licence conversation. The commercial licence starts with a short conversation with the
maintainer named in [MAINTAINERS.md](MAINTAINERS.md).

The code systems are licensed separately: SNOMED CT by SNOMED International,
LOINC by Regenstrief, ICD by WHO, RxNorm by NLM, and the repository ships none
of their content. You bring the release you are licensed for; UCUM and the IANA
and Unicode registries are vendored under their own licences, recorded beside
the data.

## Contributing

[`CONTRIBUTING.md`](CONTRIBUTING.md) has the rules; the open issues are the
worklist. Every change ships with tests, and conformance-facing behaviour cites
the FHIR or SNOMED specification it implements.

# What FerroTERM is

FerroTERM is a read-oriented FHIR terminology server for clinical code
systems. It answers the HL7 FHIR terminology operations over the code systems
you load, and every code system reaches the operations through the same
provider seam, so LOINC, ICD-10, RxNorm, and the rest are loaders and
providers, never special cases in an operation.

<!-- toc -->

## What it does

The server exposes the FHIR terminology API on R4B:

- `CodeSystem/$lookup`: a concept's display, designations by language, and
  properties.
- `CodeSystem/$subsumes`: whether one concept subsumes another, from the
  transitive closure.
- `CodeSystem/$validate-code` and `ValueSet/$validate-code`: whether a code is
  valid, whether its display is right (the correct one comes back when it is
  not), and, for a value set, whether the code is a member.
- `ValueSet/$expand`: the members of a value set, paged, filtered by text,
  with version pins, over loaded, inline, and request-scoped value sets.
- `ConceptMap/$translate`: the targets a code maps to, from loaded or inline
  maps.
- `metadata` and `metadata?mode=terminology`: what this deployment serves.

## What it serves

| Code system | What you provide | What the provider answers |
|---|---|---|
| SNOMED CT, any edition | the RF2 release zip | the inferred hierarchy, preferred terms per language reference set, inactive concepts marked, the edition and version URI as `version`, the `is-a` and `in` filters |
| LOINC | `Loinc_x.yy.zip` | terms, parts, answer lists, and answers as codes; the FHIR LOINC properties and filters; `/vs`, `/vs/[LL…]`, `/vs/[LP…]` |
| ICD-10 (WHO and national translations), ICPC-2 | a ClaML document | the `classified-with` tree, modifiers expanded onto their leaves, inclusion and exclusion notes as properties |
| ICD-10-CM | the CMS tabular XML and order file | codes with the period, seventh-character codes under their stem, the `valid` flag |
| ICD-11 (MMS, ICF, the Foundation) | a local deployment of the WHO ICD-API | short codes, entity URIs, postcoordination expressions validated against the axes, the scale value sets |
| RxNorm | the RRF release or the prescribable subset | the FHIR `STY`, `SAB`, `TTY`, `REL`, and `RELA` filters over typed edges, `/vs` |
| ATC/DDD | the WHO index as CSV, or the G-Standaard `BST801T` file | the five levels as a `classified-with` tree, DDDs as properties |
| DHD thesauri | the Uitleverformaat 5.0 delivery zip | a flat table with the SNOMED CT, ICD-10, DBC, and ZA links as properties and as concept maps |
| G-Standaard | the monthly release directory (`BSTnnnT` files) | the GPK, PRK, HPK, and article systems, the ladder as properties |
| FHIR `CodeSystem`, `ValueSet`, `ConceptMap` | a FHIR package or a directory of JSON | hierarchy, filters, properties, and supplements as the resources declare them |
| UCUM, BCP 47, BCP 13, ISO 3166-1 | nothing | grammar and registry validation, vendored into the binary |

[`docs/terminologies.md`](https://github.com/rubentalstra/FerroTERM/blob/main/docs/terminologies.md)
records each system's FHIR page, its licence, and exactly what the provider
implements.

## Why it exists

Most SNOMED CT terminology servers run on the JVM, and the reference server,
Snowstorm, runs on Elasticsearch. That stack works, and it asks for a lot of
memory: a machine serving an edition wants 16 to 32 GB of RAM, most of it
Elasticsearch heap, plus a search cluster to operate.

The data is smaller than that: a few hundred thousand concepts, a million
descriptions, a million relationships. FerroTERM builds an edition into one
memory-mapped index once and serves it from one process; the
[benchmarks page](benchmarks.md) has what that costs in build time, disk, and
resident memory for each code system, measured and recorded. The goal is plain:
run a clinical terminology server on hardware you already have.

## Who uses it

FerroTERM speaks the FHIR terminology API, so any FHIR client can use it: a
validator, an IG publisher, a clinical application resolving value set
bindings. One such client is
[FerroEHR](https://github.com/rubentalstra/FerroEHR), a pure-Rust openEHR
clinical data repository by the same author. FerroTERM is independent of it.

## What is not built yet

The build order is the tracker's milestones. As of v0.0.10:

- Post-coordinated SNOMED expressions (`expressions = true`) and the implicit
  concept maps (`?fhir_cm=`) are v0.0.11; ECL and every implicit value set
  are served.
- The licence-gated providers program (LOINC, ICD-10, the classifications,
  the Dutch national code systems) closes out in v0.0.10 over owner-licensed
  data; R4, R4B, R5, and the R6 ballot answer in JSON and XML today.
- Hierarchical (nested) expansion, persisted client resources, `Bundle`
  batch, `ConceptMap/$closure`, and the SNOMED implicit concept maps are
  v0.0.11.
- MRCM validation of postcoordinated SNOMED expressions is not scheduled.

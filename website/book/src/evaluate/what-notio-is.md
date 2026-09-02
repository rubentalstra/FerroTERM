# What FerroTERM is

FerroTERM is a read-oriented FHIR terminology server for clinical code systems.
It answers the HL7 FHIR terminology operations over the code systems you load,
SNOMED CT first, and it serves those operations across FHIR R4, R4B, R5, and R6
from one running server. Every code system reaches the operations through the
same engine seam, so LOINC, ICD-10, and the other systems on the roadmap are
loaders, never special cases.

<!-- toc -->

## What it does

The server exposes the FHIR terminology API:

- `CodeSystem/$lookup`: read a concept's properties and designations.
- `CodeSystem/$subsumes`: test whether one concept subsumes another.
- `CodeSystem/$validate-code` and `ValueSet/$validate-code`: check that a code is
  valid and, for a value set, that it is a member.
- `ValueSet/$expand`: expand a value set, driven by SNOMED's Expression
  Constraint Language (ECL).
- `ConceptMap/$translate`: translate a code through a map.

The result is a single static binary that speaks the FHIR terminology API to any
FHIR client, with a footprint small enough to run beside other services.

> [!NOTE]
> The server is in design. The operation list above is the terminology surface
> FerroTERM implements, not a surface you can call today. See the
> [Introduction](../introduction.md) for the discovery status.

## Why it exists

Most SNOMED CT terminology servers run on the JVM, and the reference server,
Snowstorm, runs on Elasticsearch. That stack works, and it asks for a lot of
memory: a machine serving the full International edition wants 16 to 32 GB of
RAM, most of it Elasticsearch heap, plus a search cluster to operate.

The data itself is smaller than that. The International edition is about 360,000
concepts, 1.2 million descriptions, and 1.5 million relationships. SNOMED
International's own lightweight server, Snowstorm Lite, drops Elasticsearch for a
single Lucene index and runs the full edition in about 500 MB. FerroTERM aims to run
the same edition in a few hundred megabytes, on ordinary hardware such as a
laptop or a small box.

The goal is plain: run a SNOMED CT terminology server on hardware you already
have.

## Who uses it

FerroTERM speaks the FHIR terminology API, so any FHIR client can use it. One such
client is [FerroEHR](https://github.com/rubentalstra/FerroEHR), a pure-Rust
openEHR clinical data repository by the same author, which resolves archetype
value-set bindings against an external FHIR terminology server. FerroTERM is
independent of FerroEHR and was not derived from it.

## What is not built yet

FerroTERM implements the whole FHIR terminology surface as a read-only server,
starting with the SNOMED CT International edition. Some capabilities come later in the build
order, so a given release may not have them yet:

- Full MRCM validation of post-coordinated expressions comes later. Today
  `$validate-code` checks pre-coordinated codes, displays, and value-set
  membership.
- `ConceptMap/$closure` (client-side closure maintenance) comes later.
- CodeSystem supplements beyond echoing the parameter come later.
- Implementation starts with R4B, and the rest of the R4/R4B/R5/R6 matrix
  follows.
- SNOMED CT is the first code system; LOINC and the other systems recorded in
  the repository's `docs/terminologies.md` follow in that build order.

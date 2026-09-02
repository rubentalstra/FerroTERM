# Nomenclator

A pure-Rust SNOMED CT terminology server that speaks the HL7 FHIR terminology
API. No JVM, no Elasticsearch.

> Status: early design. This repository is being scoped before implementation.
> The README describes the intent and the shape of the work, not a finished
> server.

## Why this exists

SNOMED CT terminology servers today are almost all Java, and the reference
server (Snowstorm) runs on Elasticsearch. That stack is capable, and it is
heavy: a machine serving the full International edition wants 16 to 32 GB of
RAM, most of it Elasticsearch heap, plus a search cluster to operate and keep
alive.

The data itself does not need that. The International edition is about 360,000
concepts, 1.2 million descriptions, and 1.5 million relationships. Held in
compact structures it fits in a few gigabytes. Nomenclator loads a SNOMED CT
RF2 release into memory-efficient Rust structures and answers terminology
queries from them, so the full International edition can run in roughly 2 to
4 GB on one small box instead of a dedicated 32 GB one.

Search uses [Tantivy](https://github.com/quickwit-oss/tantivy), a full-text
search library written in Rust and licensed Apache-2.0, in place of an external
Elasticsearch cluster. The concept graph uses roaring bitmaps for ancestor and
descendant sets, so subsumption tests are fast without a full transitive-closure
table on disk.

The goal is a terminology server that a small team can actually run: one open
source binary, no separate search service, and a memory footprint that fits a
sandbox or a modest production node.

## What it is

- A read-oriented terminology server for SNOMED CT.
- An HL7 FHIR R4 terminology API: `CodeSystem/$lookup`,
  `CodeSystem/$subsumes`, `CodeSystem/$validate-code`, `ValueSet/$expand`,
  `ValueSet/$validate-code`, and `ConceptMap/$translate`.
- Value-set expansion driven by SNOMED's Expression Constraint Language (ECL).
- A single static binary with an embedded search index. No JVM, no separate
  search service, no external database to stand up.

## What it is not, for now

- Not a SNOMED authoring server. It serves a released edition; it does not edit
  content or manage authoring branches.
- Not a drop-in replacement for Snowstorm in every setting. Multiple editions,
  historical associations for inactivated concepts, and the twice-yearly release
  lifecycle are roadmap items, not day one.
- Not a bundle of SNOMED CT content. See Licensing.

## The plan

The first milestone is a read-only core: load an RF2 snapshot, then answer
`$lookup`, `$subsumes`, and `$validate-code`. `$expand` over a first slice of
ECL follows. Correctness is checked against Snowstorm as the reference server,
because a terminology server that is subtly wrong is worse than none.

ECL is the hard part and the main risk. A correct ECL evaluator (refinements,
attribute groups, cardinality, dotted attributes) is where the real work sits,
so it is built and tested as its own layer against the published ECL grammar
before the value-set surface depends on it.

## Design notes

- Concepts, descriptions, and relationships load from RF2 into interned,
  integer-keyed structures.
- The is-a graph is a DAG, because SNOMED allows a concept more than one
  parent, so ancestor and descendant sets are stored as roaring bitmaps rather
  than a tree labeling.
- Description search runs through Tantivy, with language reference sets and
  acceptability taken into account for ranking and filtering.
- The whole server is read-mostly once an edition is loaded, which is what
  keeps the memory footprint small and the operational surface flat.

## Licensing

Two separate things, and they must not be confused:

- **The software** in this repository is open source under the MIT license.
  Use it, embed it, run it.
- **SNOMED CT content** is licensed by SNOMED International and is not
  distributed here. Running Nomenclator against the International edition
  requires a valid SNOMED CT licence, which is free within member countries
  (the Netherlands among them) and available under the affiliate licence
  elsewhere. You bring your own RF2 release and the server loads it.

## Relationship to FerroEHR

Nomenclator is a standalone project. It grew out of
[FerroEHR](https://github.com/rubentalstra/FerroEHR), a pure-Rust openEHR
clinical data repository, which resolves archetype value-set bindings against an
external FHIR terminology server. openEHR places that resolver outside the CDR
by design, so Nomenclator is a separate server reached over FHIR rather than a
part of the CDR, and it is useful to any FHIR client, not only to FerroEHR.

## The name

SNOMED CT stands for Systematized Nomenclature of Medicine, Clinical Terms. A
nomenclator is one who assigns and announces names. The server's job is to know
what each code means and how the codes relate to each other.

## Contributing

The project is at the design stage, so the most useful contributions right now
are review of the scope above and of the ECL approach in particular. Issues and
discussion are welcome.

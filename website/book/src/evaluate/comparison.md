# How it compares

FerroTERM serves the same FHIR terminology API as the established servers. The
difference is the runtime footprint and the stack it needs to operate. This page
sets FerroTERM beside Snowstorm, Ontoserver, and Hermes.

> [!NOTE]
> The numbers for the other servers come from their own documentation and
> published work, cited in
> [`docs/architecture.md`](https://github.com/rubentalstra/FerroTERM/blob/main/docs/architecture.md).
> FerroTERM's own figures are design targets, since the server is in design.

## The servers

- **Snowstorm** (SNOMED International): Java on Elasticsearch. The reference
  server and FerroTERM's correctness oracle. It is full-featured, and a full
  International edition deployment wants 16 to 32 GB of RAM plus a search cluster.
- **Snowstorm Lite** (SNOMED International): drops Elasticsearch for a single
  Lucene index and runs the full International edition in about 500 MB. It is the
  closest point of comparison for footprint.
- **Ontoserver** (CSIRO): Postgres plus Lucene. A production FHIR terminology
  server, index-materialized rather than a graph database.
- **Hermes** (Mark Wardle): Clojure, a memory-mapped store plus Lucene. It serves
  subsumption in tens of microseconds from a materialized structure, which is the
  design point FerroTERM follows in pure Rust.

## Side by side

| Server | Language / runtime | Store | Full-edition memory | Deploys as |
|---|---|---|---|---|
| Snowstorm | Java (JVM) | Elasticsearch | 16 to 32 GB | JVM plus a search cluster |
| Snowstorm Lite | Java (JVM) | Lucene | about 500 MB | JVM service |
| Ontoserver | Java (JVM) | Postgres plus Lucene | server plus database | JVM plus Postgres |
| Hermes | Clojure (JVM) | memory-mapped store plus Lucene | modest | JVM service |
| FerroTERM (planned) | Rust | `redb`, memory-mapped | a few hundred MB target | single static binary |

## What FerroTERM trades

FerroTERM aims for a small footprint and a single binary, and it starts as a
read-oriented server over a loaded edition. The mature servers carry features
that come later in FerroTERM's build order, such as full MRCM validation of
post-coordinated expressions and closure maintenance. See what is not built yet
in [What FerroTERM is](what-ferroterm-is.md).

Every production server in this list converges on a materialized index rather
than live graph traversal. FerroTERM takes the same shape and implements it in pure
Rust with a machine-generated FHIR layer across four versions, which no existing
Rust project provides.

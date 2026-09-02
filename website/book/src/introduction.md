# Introduction

FerroTERM is a pure-Rust FHIR terminology server for SNOMED CT, LOINC, and other
clinical code systems, SNOMED CT first. It serves the HL7 FHIR terminology API
from a single binary, backed by a memory-mapped index built once per code
system release. There is no JVM, no Elasticsearch, and no external
database.

> [!NOTE]
> The name: Ferro for the Rust family FerroTERM shares with FerroEHR, TERM for
> terminology. The site is <https://ferroterm.eu>.

## Status: early design

This repository is in discovery. The team is scoping the design before writing
product code, so this book describes intended behaviour, marked as planned where
the server does not exist yet. Read a sentence in the future tense ("the server
returns", "you run") as a design commitment, not a claim that you can run it
today. The design authority, with citations, is
[`docs/architecture.md`](https://github.com/rubentalstra/FerroTERM/blob/main/docs/architecture.md)
in the repository.

## Who each part of this book is for

The book is organized by what you want to do.

- **[Evaluate](evaluate/what-ferroterm-is.md)** is for anyone deciding whether FerroTERM
  fits. It covers what the server is, why it exists, the architecture at a
  glance, and how it compares to Snowstorm, Ontoserver, and Hermes.
- **[Operate](operate/install.md)** is for operators and deployers. It covers
  installing and running the server, configuration, loading a licensed SNOMED CT
  edition, hardware sizing, and verifying a release before you run it.
- **[Integrate](integrate/fhir-api.md)** is for API consumers. It covers the
  FHIR terminology operations, implicit value sets and ECL, the supported FHIR
  versions, and worked request and response examples.
- **[Contribute](contribute/build-and-test.md)** is for contributors. It covers
  building and testing, the FHIR code generation model, and where the deeper
  design lives.

## Two things that are always true

You bring your own code system content. The software is open source under the
MIT license. SNOMED CT is licensed by SNOMED International, LOINC by the
Regenstrief Institute, and this repository ships no code system content. You
load a licensed release, and the server serves it. See [Loading a SNOMED CT edition](operate/loading-snomed.md).

# Licensing

FerroTERM is source-available under the Business Source License 1.1. The
`LICENSE` file in the repository is the authority; this page states the same
boundary in the order people ask about it, so you can answer "do I need a
commercial licence?" without reading the parameters.

The code systems are a separate question, answered at the bottom of this page.

## Do I need a commercial licence?

| You are | What you are doing | Licence |
|---|---|---|
| Anyone | Reading, building, modifying, or redistributing the source | Free |
| Anyone | Development, testing, evaluation, prototyping | Free |
| A person | Personal use | Free |
| A university or research group | Academic or scientific research, or teaching | Free |
| A non-profit or public body | Production use that is not in the course of a business, does not deliver a service for payment, and is not for commercial advantage | Free |
| A hospital, clinic, or care provider | Treating patients | Commercial licence |
| A vendor or integrator | Any production use | Commercial licence |
| Anyone | Offering FerroTERM, or a work derived from it, to third parties as a hosted, managed, or embedded terminology service | Commercial licence |
| Anyone | Selling, sublicensing, or otherwise distributing it for a fee, on its own or inside another product | Commercial licence |

The last two rows hold whatever else you are. Hosting FerroTERM for third
parties, and distributing it for a fee, need a commercial licence in every
case, including for an organisation the rows above would otherwise leave free.

## What the licence says

Reading, building, modifying, and redistributing the source is free, without a
fee and without asking anyone, and every non-production use is covered:
development, testing, evaluation, and prototyping.

Production use is free for Non-Commercial Purposes, which the licence defines
as personal use, academic or scientific research, teaching, and use by a
non-profit organisation or public body that is not in the course of a business,
does not deliver a service for payment, and is not for commercial advantage.

Any other production use needs a commercial licence from the Licensor,
including the delivery of health care or any other service for payment.

There is no open-core tier. The engine, the server, and the tools are in one
repository under one licence, and nothing is held back to be sold back to you.

## When it becomes Apache 2.0

Each version becomes Apache License 2.0 four years after that version is
published. The clock runs per version, so the four years start at the release
you are looking at rather than at the project.

## The two crates that are Apache 2.0 today

`fhir-types` (the FHIR types and operation contracts generated from HL7's own
packages) and `rf2` (the SNOMED CT release file reader) are published to
crates.io under Apache License 2.0 rather than the Business Source License, so
any Rust project can depend on them without a licence conversation. Every other
published crate carries the Business Source License and states so in its own
README.

## Starting a commercial licence

It starts with a short conversation with the maintainer named in
[MAINTAINERS.md](https://github.com/rubentalstra/FerroTERM/blob/main/MAINTAINERS.md).
Companies and care providers building on FerroTERM are wanted here, and the
commercial licence is the normal path for them.

## The code systems are licensed separately

The licence above covers the software. The content of each code system is
licensed by its publisher: SNOMED CT by SNOMED International, LOINC by
Regenstrief, ICD by the WHO, RxNorm by the NLM. The repository ships none of
it, and a deployment brings the release it is licensed for. UCUM and the IANA
and Unicode registries are vendored under their own licences, recorded beside
the data.

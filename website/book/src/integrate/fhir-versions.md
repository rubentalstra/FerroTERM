# FHIR versions

Notio serves the FHIR terminology API across four versions from one running
server: R4, R4B, R5, and R6. A client picks the version, and the server answers
in that version's shapes.

<!-- toc -->

## Why four versions from one server

HL7 publishes the whole type system and every operation as machine-readable
resources, in versioned packages. Notio vendors and pins those packages and
generates per-version Rust modules from them, so each version's operation surface
is correct by construction. An operation parameter that R5 adds appears in the R5
module and is absent from R4B, because the generator emits what each package
declares. A runtime wrapper routes a request to the module for its version, so
one server answers all four callers at once.

## Version status

| Version | Package | Status in Notio |
|---|---|---|
| R4B | `hl7.fhir.r4b.core` 4.3.0 | first generation implemented |
| R5 | `hl7.fhir.r5.core` 5.0.0 | follows R4B |
| R4 | `hl7.fhir.r4.core` 4.0.1 | follows R4B |
| R6 | `hl7.fhir.r6.core` 6.0.0-ballot | ballot-tracking, follows the others |

> [!NOTE]
> R4B is the first version implemented. It is the current stable release of the
> R4 line and a near-superset of R4, so an R4B-first build already serves the
> R4-family terminology surface. R5, R4, and R6 follow as further generations.
> R6 tracks the ballot, with publication expected around late 2026.

## What this means for you

Send your request in the FHIR version your client uses. The operation names,
parameters, and response shapes are the ones the FHIR specification defines for
that version, so you do not adapt your client to Notio. Implementation starts
with R4B, and the other versions become available as they are implemented. The
CapabilityStatement at `/metadata` reports which versions a given deployment
serves.

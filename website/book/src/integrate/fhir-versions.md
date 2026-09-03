# FHIR versions

FerroTERM answers FHIR R4B today, under `/r4b`. The R4, R5, and R6 modules are
generated and the endpoints that serve them are the v0.0.9 milestone.

<!-- toc -->

## Why one server can serve four versions

HL7 publishes the whole type system and every operation as machine-readable
resources, in versioned packages. FerroTERM vendors and pins those packages and
generates per-version Rust modules from them, so each version's operation
surface is correct by construction. An operation parameter that R5 adds appears
in the R5 module and is absent from R4B, because the generator emits what each
package declares. The engine beneath is version-neutral; a version's endpoint
maps the engine's answers into that version's shapes.

## Version status

| Version | Package | Status |
|---|---|---|
| R4B | `hl7.fhir.r4b.core` 4.3.0 | served under `/r4b` |
| R5 | `hl7.fhir.r5.core` 5.0.0 | generated; the `/r5` endpoint is v0.0.9 |
| R4 | `hl7.fhir.r4.core` 4.0.1 | generated; the `/r4` endpoint is v0.0.9 |
| R6 | `hl7.fhir.r6.core` 6.0.0-ballot5 | generated; the `/r6` endpoint is v0.0.9, marked ballot-tracking |

R4B is the current stable release of the R4 line and a near-superset of R4,
so the R4B endpoint already serves the R4-family terminology surface to
clients that accept 4.3 shapes. The R5 shapes matter most to validators and IG
tooling (`$validate-code` `issues`, `$lookup` `subproperty`, `$expand`
`property`), which is why the R5 endpoint is first in that milestone.

## What this means for you

Send R4B requests to `/r4b`. `GET /r4b/$versions` names the version of the
base (`4.3`), `GET /r4b/metadata` is the R4B `CapabilityStatement`, and a
parameter R4B does not declare is refused with an `OperationOutcome` rather
than absorbed. When the other endpoints land, each answers in its own version's
shapes at its own base.

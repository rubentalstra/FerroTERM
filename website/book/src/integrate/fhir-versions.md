# FHIR versions

FerroTERM answers FHIR R4 under `/r4` and FHIR R4B under `/r4b`. The R5 and R6
modules are generated and the endpoints that serve them are the v0.0.9
milestone.

<!-- toc -->

## Why one server can serve four versions

HL7 publishes the whole type system and every operation as machine-readable
resources, in versioned packages. FerroTERM vendors and pins those packages and
generates per-version Rust modules from them, so each version's operation
surface is correct by construction. An operation parameter that R5 adds appears
in the R5 module and is absent from R4 and R4B, because the generator emits
what each package declares. The engine beneath is version-neutral; a version's
endpoint maps the engine's answers into that version's shapes. R4 and R4B
declare the same terminology parameters and elements, so one set of macros
instantiates both endpoints from their own generated modules and they cannot
drift apart.

## Version status

| Version | Package | Status |
|---|---|---|
| R4 | `hl7.fhir.r4.core` 4.0.1 | served under `/r4` |
| R4B | `hl7.fhir.r4b.core` 4.3.0 | served under `/r4b` |
| R5 | `hl7.fhir.r5.core` 5.0.0 | generated; the `/r5` endpoint is v0.0.9 |
| R6 | `hl7.fhir.r6.core` 6.0.0-ballot5 | generated; the `/r6` endpoint is v0.0.9, marked ballot-tracking |

R4B is the current stable release of the R4 line and a near-superset of R4;
both endpoints serve the same engine, and a `$cache-control` cache started on
one serves the other. The R5 shapes matter most to validators and IG
tooling (`$validate-code` `issues`, `$lookup` `subproperty`, `$expand`
`property`), which is why the R5 endpoint is first in that milestone.

## What this means for you

Send R4 requests to `/r4` and R4B requests to `/r4b`. `GET {base}/$versions`
names the version of the base (`4.0` or `4.3`), `GET {base}/metadata` is that
version's `CapabilityStatement`, and a parameter the version does not declare
is refused with an `OperationOutcome` rather than absorbed. When the R5 and R6
endpoints land, each answers in its own version's shapes at its own base.

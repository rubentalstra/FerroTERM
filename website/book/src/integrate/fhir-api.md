# The FHIR terminology API

FerroTERM speaks the HL7 FHIR terminology API. If your client already talks to a FHIR
terminology server, it talks to FerroTERM. This page lists the operations and what
each one answers.

> [!NOTE]
> The server is in design. The operations below are the terminology surface FerroTERM
> implements, and their FHIR shapes are fixed by the specification, but you cannot
> call a running server yet. Requests and responses are marked as planned on the
> [Worked examples](examples.md) page.

<!-- toc -->

## The operations

| Operation | Answers | Structure that serves it |
|---|---|---|
| `CodeSystem/$lookup` | a concept's properties and designations | columnar store plus adjacency |
| `CodeSystem/$subsumes` | does concept A subsume concept B | roaring-bitmap membership |
| `CodeSystem/$validate-code` | is this a valid code and display | columnar store |
| `ValueSet/$validate-code` | is this code a member of the value set | store plus expansion |
| `ValueSet/$expand` | the members of a value set, ECL-driven | precomputed bitmaps and set algebra |
| `ConceptMap/$translate` | the targets a code maps to | map-refset lookup |

The FHIR specification defines the parameters and the response shape for each
operation. FerroTERM follows the specification for the FHIR version in the request.

## How a request is shaped

Every operation is a FHIR operation invocation, called with `GET` for simple
parameters or `POST` with a `Parameters` resource for complex input. Results come
back as a `Parameters` resource, or the operation's defined resource for
`$expand` (an expanded `ValueSet`). Errors come back as an `OperationOutcome`
with a diagnostic issue.

For example, `$lookup` takes a `system` and a `code` and returns the concept's
name, its designations, and its requested properties. `$subsumes` takes two codes
and a `system` and returns whether the first subsumes the second, the reverse, is
equivalent, or is unrelated.

## Request-scoped resources

A client can bring its own `CodeSystem` and `ValueSet` resources for one
request: each `tx-resource` parameter of a `POST` carries one, and the
operation sees them layered over the loaded resources (a resource with the
same `url` and `version` as a loaded one shadows it for that request). A
supplement in a `tx-resource` applies to the system it names. Any other
resource type is refused with `not-supported`. This is what the HL7
terminology ecosystem runner sends; `tx-resource` is a declared parameter in
R6 only and an ecosystem extension on R4B
(<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/requirements.html>).

To avoid resending the same resources, start a cache:

```text
POST [base]/$cache-control?mode=start   (Parameters of tx-resource)
→ Parameters { cache-id }
```

Then name it with the `X-Cache-Id` header on later requests; their own
`tx-resource`s stack on top. `POST [base]/$cache-control?mode=end` with the
header (or a `cache-id` parameter) releases it. A cache unused for 30 minutes
expires, and an unknown id answers `404`.

`GET [base]/$versions` (R5's `CapabilityStatement/$versions`) names the FHIR
version of the base as `MAJOR.MINOR`. Loaded value sets are readable as
`GET [base]/ValueSet/{id}` and searchable as `GET [base]/ValueSet?url=…&version=…`.

## SNOMED CT as the code system

For SNOMED CT the `system` is the SNOMED CT URI, `http://snomed.info/sct`. An
edition and version are addressed with the SNOMED CT URI standard's `version`
form. The implicit value sets and ECL that drive `$expand` are covered on
[Implicit value sets and ECL](ecl-value-sets.md).

## Metadata

A client discovers what a deployment serves from its CapabilityStatement at
`/metadata`, and from the TerminologyCapabilities resource. These report the FHIR
versions this server answers and the terminology operations it supports.

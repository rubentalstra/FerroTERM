# Worked examples

This page shows request and response shapes for the common operations. The FHIR
shapes are fixed by the specification. The values are illustrative.

> [!WARNING]
> These examples are planned. FerroTERM has no running server yet, so you cannot send
> these requests today. The request and response shapes follow the FHIR
> terminology specification, and the concrete codes and displays are placeholders.

<!-- toc -->

## $lookup a concept

Read a concept's display and properties:

```http
GET /CodeSystem/$lookup?system=http://snomed.info/sct&code=73211009
```

A `Parameters` response carries the name, the display, and the requested
properties:

```json
{
  "resourceType": "Parameters",
  "parameter": [
    { "name": "name", "valueString": "SNOMED CT" },
    { "name": "display", "valueString": "Diabetes mellitus (disorder)" }
  ]
}
```

## $subsumes two concepts

Test whether one concept subsumes another:

```http
GET /CodeSystem/$subsumes?system=http://snomed.info/sct&codeA=73211009&codeB=44054006
```

The response reports the relationship, one of `subsumes`, `subsumed-by`,
`equivalent`, or `not-subsumed`:

```json
{
  "resourceType": "Parameters",
  "parameter": [
    { "name": "outcome", "valueCode": "subsumes" }
  ]
}
```

## $validate-code against a value set

Check that a code is a member of a value set:

```http
GET /ValueSet/$validate-code?url=http://snomed.info/sct?fhir_vs=isa/73211009&system=http://snomed.info/sct&code=44054006
```

```json
{
  "resourceType": "Parameters",
  "parameter": [
    { "name": "result", "valueBoolean": true }
  ]
}
```

## $expand an ECL value set

Expand an implicit value set named by ECL, one page at a time:

```http
GET /ValueSet/$expand?url=http://snomed.info/sct?fhir_vs=ecl/<<73211009&count=20&offset=0
```

The response is an expanded `ValueSet` whose `expansion.contains` holds the page
of members, with `total` reporting the full set size:

```json
{
  "resourceType": "ValueSet",
  "expansion": {
    "total": 137,
    "offset": 0,
    "contains": [
      { "system": "http://snomed.info/sct", "code": "73211009", "display": "Diabetes mellitus (disorder)" }
    ]
  }
}
```

## An error

An invalid or unknown input returns an `OperationOutcome`:

```json
{
  "resourceType": "OperationOutcome",
  "issue": [
    { "severity": "error", "code": "code-invalid", "diagnostics": "Unknown code '00000' in system 'http://snomed.info/sct'." }
  ]
}
```

See [The FHIR terminology API](fhir-api.md) for the operation list and
[Implicit value sets and ECL](ecl-value-sets.md) for the ECL URL convention.

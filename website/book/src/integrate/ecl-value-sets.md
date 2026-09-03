# Value sets, implicit value sets, and ECL

A value set names a set of codes. FerroTERM answers value sets defined as FHIR
resources, brought with the request, or implied by a URL a code system
defines. This page covers what is answered today and what the ECL milestone
adds.

<!-- toc -->

## Implicit value sets today

Several code systems define value sets by URL convention, without a stored
resource, and their providers answer them:

| Code system | URL | Members |
|---|---|---|
| LOINC | `http://loinc.org/vs` | every code |
| LOINC | `http://loinc.org/vs/LL…` | the answers of the answer list |
| LOINC | `http://loinc.org/vs/LP…` | everything under the part in the multiaxial hierarchy |
| UCUM | `http://unitsofmeasure.org/vs` | every valid unit (validation only; it cannot be enumerated) |
| RxNorm | `http://www.nlm.nih.gov/research/umls/rxnorm/vs` | every RXCUI |
| ICD-11 | `<entity URI>/postcoordinationScale/<axis>` | the values a stem takes on that axis |

Pass the URL as `url` to `ValueSet/$expand` or `ValueSet/$validate-code`.

## A SNOMED CT value set today

The FHIR SNOMED CT page defines `http://snomed.info/sct?fhir_vs` (all
concepts), `?fhir_vs=isa/[sctid]`, `?fhir_vs=refset`, `?fhir_vs=refset/[sctid]`,
and `?fhir_vs=ecl/[expression]`, with an optional edition base. FerroTERM does
not answer these URLs yet; the first four are tracked for the current
milestone and `ecl/` for v0.0.8. Until then a SNOMED CT value set is a
`compose` with the filters the provider declares, sent inline or as a loaded
resource:

```json
{
  "resourceType": "ValueSet",
  "status": "active",
  "compose": {
    "include": [{
      "system": "http://snomed.info/sct",
      "filter": [{ "property": "concept", "op": "is-a", "value": "404684003" }]
    }]
  }
}
```

The `is-a`, `descendent-of`, `is-not-a`, `generalizes`, `in`, `not-in`, and
`regex` filters over `concept` are answered from the transitive closure and the
store; a display language selects the preferred term from the language
reference set.

## What ECL adds (v0.0.8)

Every ECL expression returns a set of concepts, and the evaluator compiles it
to set algebra over the precomputed bitmaps: `<< X` is the descendant bitmap
plus `X`, a refinement such as `< 404684003 : 363698007 = << 39057004`
intersects with the attribute adjacency, and `AND`, `OR`, and `MINUS` are
bitmap operations. The parser follows the official ANTLR grammar for ECL 2.2
rule for rule and is tested against the published valid and invalid corpus;
the evaluator is checked against Snowstorm over the same edition. The three
issues of the milestone are the parser, the evaluator, and `?fhir_vs=ecl/`
together with the `constraint` filter.

## Paging a large expansion

A broad expansion names a large set, so `$expand` pages with `count` and
`offset` and refuses an unpaged expansion beyond 1,000 members with
`too-costly`. `expansion.total` reports the full size, `expansion.offset` the
page, and the order is deterministic across calls.

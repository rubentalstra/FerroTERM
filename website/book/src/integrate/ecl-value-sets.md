# Implicit value sets and ECL

SNOMED CT defines implicit value sets: value sets you name by a URL convention
rather than by storing a ValueSet resource. Notio expands them with SNOMED's
Expression Constraint Language (ECL). This page explains the URL convention and
how ECL drives an expansion.

> [!NOTE]
> ECL is the hard part of the server and its main risk, so it is built and tested
> as its own layer against the published ECL grammar before the value-set surface
> depends on it. Correctness is measured against Snowstorm.

<!-- toc -->

## The implicit value set convention

SNOMED CT on FHIR lets you name a value set by an ECL expression in the value
set's URL, using the `fhir_vs` convention:

```text
http://snomed.info/sct?fhir_vs=ecl/<expression>
```

You pass that URL as the value set to `ValueSet/$expand`, and the server expands
the ECL expression against the loaded edition. The plain `?fhir_vs` (with no
`ecl/`) names the implicit value set of all SNOMED CT concepts, and `?fhir_vs=isa/<code>`
names the value set of a concept and its descendants.

## What ECL expresses

Every ECL expression returns a set of concepts. The core operators over the is-a
hierarchy:

| ECL | Meaning |
|---|---|
| `<< X` | X and all its descendants |
| `< X` | the descendants of X, not X itself |
| `>> X` | X and all its ancestors |
| `> X` | the ancestors of X, not X itself |

Refinement narrows a set by attribute values, for example `< 404684003 : 363698007 = << 39057004`
reads as the descendants of one concept that have a given attribute pointing into
a given subtree. ECL also supports conjunction, disjunction, exclusion, attribute
groups, and cardinality.

## How Notio evaluates it

Notio compiles an ECL expression to set algebra over the precomputed structures.
A `<<` or `>>` set is a precomputed bitmap returned directly, a refinement is a
bitmap intersection or union over the per-attribute adjacency, and conjunction
and disjunction are bitmap AND and OR. There is no live graph traversal on this
path, which is what keeps a descendant expansion of a high-level concept fast.

## Paging a large expansion

A broad expansion can name a large set, so `$expand` pages its results with the
FHIR `count` and `offset` parameters, bounded by the server's configured maximum
page size (see [Configuration](../operate/configuration.md)). Request a page at a
time rather than an unbounded expansion.

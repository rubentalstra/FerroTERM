# notio-graph

The materialized ontology. Hand-written; no FHIR or SNOMED spec governs the
storage layout (our own design, `docs/architecture.md`), while the SNOMED CT
logic profile and the ECL specification govern what the algebra must answer.

- SNOMED is a graph model served from an index: CSR adjacency arrays keyed by
  dense ordinals, and a roaring bitmap per concept for its transitive closure.
  Never a pointer graph, never live traversal on a request path.
- The subsumption and set operations here are the only place ECL semantics
  turn into bitmap operations; keep them total and deterministic (roaring
  iterates in sorted order).
- Load-bearing ordinal arithmetic uses `checked_*` with a typed error, never a
  bare cast or index (`.claude/rules/reliability.md`).

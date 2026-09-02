# ferroterm-graph

The materialized hierarchy of a loaded code system. Hand-written; no FHIR or
SNOMED spec governs the storage layout (our own design, `docs/architecture.md`),
while each code system's own specification (the SNOMED CT logic profile and
ECL, the LOINC multiaxial hierarchy, the ICD chapter tree) governs what the
algebra must answer.

- Code-system-neutral: the graph knows dense concept ordinals and typed edges,
  never SCTIDs, LOINC parts, or ICD chapters. A loader (`ferroterm-rf2` and the
  loaders that follow it) maps its release into ordinals and edge types.
- A graph model served from an index: CSR adjacency arrays keyed by ordinal,
  and a roaring bitmap per concept for its transitive closure. Never a pointer
  graph, never live traversal on a request path.
- Subsumption and the set operations here are the only place hierarchy
  semantics turn into bitmap operations; keep them total and deterministic
  (roaring iterates in sorted order). ECL compiles to these operations; a
  system without a hierarchy (UCUM) simply has no closure.
- Load-bearing ordinal arithmetic uses `checked_*` with a typed error, never a
  bare cast or index (`.claude/rules/reliability.md`).

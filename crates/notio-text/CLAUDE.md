# notio-text

The description search index. Hand-written; no spec governs the index format
(our own design). The FHIR `$expand` `filter` parameter semantics and the
SNOMED search guidance govern what a query must return.

- Per-word prefix matching over an `fst` set; posting lists are roaring
  bitmaps of description ordinals.
- Result order is deterministic: matched-term length, then description id.
  `$expand` paging depends on it.
- Language and refset filters are bitmap intersections, never a post-filter
  loop over strings.
- Fixtures are invented terms over synthetic ids only.

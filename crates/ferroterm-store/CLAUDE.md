# ferroterm-store

The memory-mapped concept store. Hand-written; no spec governs the on-disk
layout (our own design, `docs/architecture.md` decision 3). `redb` owns the
memory mapping and the `unsafe`; this crate stays safe.

- Modules: `tables` (the table set and `META` keys), `record` (the byte
  encodings of concepts, designations, and typed property values, decoded with
  typed errors), `builder` (one write transaction per artifact, the
  precomputed preferred designations, a deterministic commit), `store`
  (read-only point reads).
- Code-system-neutral: a store holds one code system version keyed by dense
  ordinal with the system's native code as the string key; property keys,
  designation uses, language reference sets, and acceptabilities are small
  vocabulary tables the loader fills. SNOMED's SCTIDs, LOINC's codes, and
  ICD's categories all fit the same tables.
- Values are byte strings decoded by `record`, never redb-typed structs:
  redb's `from_bytes` cannot fail, and a damaged artifact must be a typed
  error, not a panic.
- Point reads only on the request path; the one scan (`vocabulary_ordinal`)
  walks a table of a few dozen rows. Whole-table work is the offline build.
- The store is opened read-only by the server (`ReadOnlyDatabase`); the
  writer is the offline build.
- Fixtures are synthetic stores in a temporary directory; the ignored
  `local_edition` test builds the licensed release under `data/`.

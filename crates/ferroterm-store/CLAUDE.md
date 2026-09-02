# ferroterm-store

The memory-mapped concept store. Hand-written; no spec governs the on-disk
layout (our own design). `redb` owns the memory mapping and the `unsafe`; this
crate stays safe.

- Code-system-neutral: a store holds one code system version (identity,
  concepts, displays, designations by language, typed property values) keyed
  by dense ordinal, with the system's native code as a string key. SNOMED's
  SCTIDs, LOINC's codes, and ICD's categories all fit the same tables; the
  loader supplies the property vocabulary.
- Point reads only on the request path. A scan is a build-time operation in
  `ferroterm-build`, never something a FHIR operation does.
- The store is opened read-only by the server; the writer is the offline
  build.
- No licensed content in fixtures: tests build a tiny synthetic store in a
  temporary directory.

# notio-store

The memory-mapped store. Hand-written; no spec governs the on-disk layout
(our own design). `redb` owns the memory mapping and the `unsafe`; this crate
stays safe.

- Tables are keyed by the typed ids from `notio-rf2`; a table never mixes id
  kinds.
- Point reads only on the request path. A scan is a build-time operation in
  `notio-build`, never something a FHIR operation does.
- The store is opened read-only by the server; the writer is the offline
  build.
- No SNOMED content in fixtures: tests build a tiny synthetic store in a
  temporary directory.

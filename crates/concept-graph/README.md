# concept-graph

The materialized hierarchy of a code system.

Integer-keyed compressed sparse row adjacency for a code system's is-a
hierarchy and each typed relationship, plus roaring transitive-closure bitmaps.
Subsumption is a bitmap membership test, and the ECL evaluator compiles
constraints to set algebra over these bitmaps. The graph is built offline by a
loader and served read-only; no request traverses edges live. No FHIR or SNOMED
specification governs the layout: it is this project's own design.

## Where it sits

`concept-graph` is one crate of [FerroTERM](https://github.com/rubentalstra/FerroTERM),
a pure-Rust FHIR terminology server for SNOMED CT, LOINC, and other clinical
code systems. The crates are published so other projects can reuse them; the
API is pre-1.0 and moves with the FerroTERM release train. Documentation:
<https://docs.rs/concept-graph>.

## Licence

Apache License, Version 2.0 (`LICENSE`). Clinical terminology content (SNOMED
CT, LOINC, RxNorm, ICD, the Dutch national code systems) is licensed by its
publisher and is never part of this crate.

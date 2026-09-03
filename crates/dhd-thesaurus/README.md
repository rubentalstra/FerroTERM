# dhd-thesaurus

The DHD Diagnosethesaurus and Verrichtingenthesaurus deliveries.

Reads a DHD thesaurus delivery in the "Uitleverformaat 5.0" (a set of UTF-8 CSV
tables) into typed rows: the concepts with their flags and validity dates, the
terms by type and language, and the mappings to SNOMED CT and LOINC. The crate
ships no DHD content; a deployment brings its own licensed delivery.

## Where it sits

`dhd-thesaurus` is one crate of [FerroTERM](https://github.com/rubentalstra/FerroTERM),
a pure-Rust FHIR terminology server for SNOMED CT, LOINC, and other clinical
code systems. The crates are published so other projects can reuse them; the
API is pre-1.0 and moves with the FerroTERM release train. Documentation:
<https://docs.rs/dhd-thesaurus>.

## Licence

Apache License, Version 2.0 (`LICENSE`). Clinical terminology content (SNOMED
CT, LOINC, RxNorm, ICD, the Dutch national code systems) is licensed by its
publisher and is never part of this crate.

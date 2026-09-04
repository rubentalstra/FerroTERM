# labcodeset

The Nederlandse Labcodeset publication.

Reads the Labcodeset publication Nictiz distributes (the `labconcepts-*.xml`
document of a release) into a typed model: the laboratory concepts with their
LOINC axes and Dutch translations, the SNOMED CT materials and outcome lists,
the UCUM units, and the ordinal outcome value sets. The crate ships no
Labcodeset content; a deployment brings its own licensed publication.

## Where it sits

`labcodeset` is one crate of [FerroTERM](https://github.com/rubentalstra/FerroTERM),
a pure-Rust FHIR terminology server for SNOMED CT, LOINC, and other clinical
code systems. The crates are published so other projects can reuse them; the
API is pre-1.0 and moves with the FerroTERM release train. Documentation:
<https://docs.rs/labcodeset>.

## Licence

Apache License, Version 2.0 (`LICENSE`). Clinical terminology content (SNOMED
CT, LOINC, UCUM, the Labcodeset) is licensed by its publisher and is never
part of this crate.

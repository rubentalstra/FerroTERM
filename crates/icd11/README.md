# icd11

ICD-11 from the WHO ICD-API.

Walks a local deployment of the WHO ICD-API and caches every entity of a
linearization (or the Foundation) as the JSON the API serves, per language;
reads such a cache back into typed entity records; and parses postcoordination
expressions. The system URIs are those of HL7's terminology repository. The
crate ships no ICD-11 content.

## Where it sits

`icd11` is one crate of [FerroTERM](https://github.com/rubentalstra/FerroTERM),
a pure-Rust FHIR terminology server for SNOMED CT, LOINC, and other clinical
code systems. The crates are published so other projects can reuse them; the
API is pre-1.0 and moves with the FerroTERM release train. Documentation:
<https://docs.rs/icd11>.

## Licence

Apache License, Version 2.0 (`LICENSE`). Clinical terminology content (SNOMED
CT, LOINC, RxNorm, ICD, the Dutch national code systems) is licensed by its
publisher and is never part of this crate.

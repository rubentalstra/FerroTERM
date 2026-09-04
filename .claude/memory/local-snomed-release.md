---
name: local-snomed-release
description: The owner's licensed SNOMED releases live under data/snomed/ (gitignored): the NL edition 20260630 and, since 2026-09-03, the International edition 20260901 for comparison and speed; LOINC 2.83 under data/loinc/; built artifacts under artifacts/{nl,int,loinc,...}
metadata:
  type: project
---

Licensed source data on this machine, never committed (`/data/` is gitignored):

- `data/snomed/SnomedCT_ManagedServiceNL_PRODUCTION_NL1000146_20260630T120000Z.zip` (and the unpacked directory): the Dutch edition, `http://snomed.info/sct/11000146104/version/20260630`, 548,949 concepts.
- `data/snomed/SnomedCT_InternationalRF2_PRODUCTION_20260901T120000Z.zip`: the International edition September 2026 (MD5 92e46330b8e04016775ca5dd9b8c64cb), `http://snomed.info/sct/449080006/version/20260901`, 535,502 concepts; the owner added it on 2026-09-03 "for comparison, also for the speed later".
- `data/loinc/Loinc_2.83.zip`: LOINC 2.83 (MD5 057ddf203164705d5a4c3604257060a4), added 2026-09-03.
- `data/icd11/`, `data/icd10cm/`, `data/rxnorm/` as before.

Built artifacts (gitignored `/artifacts/`): `artifacts/nl`, `artifacts/int` (each about 64 to 67 s to build with the ECL files, above the 60 s ingest bar; see #125), `artifacts/loinc` (112,405 terms), `artifacts/icd11/*`, `artifacts/icd10cm`, `artifacts/rxnorm`.

**Why:** the benches and the differential checks run over these; fixtures stay synthetic ([[performance-bar]], [[milestone-autonomy]]).

**How to apply:** rebuild an artifact after a pipeline change with `cargo run --release -p ferroterm-build -- --rf2 <zip> --out artifacts/<name>`; the ECL benches read `artifacts/nl`.

Data drops added 2026-09-04, all under `data/` and never committed: `data/labcodeset/Labcodeset_v2026-08.zip` (Nictiz Labcodeset, built with `ferroterm-build --labcodeset`), `data/icpc2/SnomedCT_ICNPNursingPractice_PRODUCTION_20260331T120000Z.zip` (the ICNP refset-only RF2 package over the January 2026 International Edition; loading it is #205), and `data/nhg/ICPC-SNOMED-20260331.json` (the NHG ICPC-1 to SNOMED CT `ConceptMap`, R4; loads as a FHIR resource dir once its string `experimental` is a boolean).

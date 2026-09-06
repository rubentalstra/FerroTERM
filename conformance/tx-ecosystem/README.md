# The HL7 terminology ecosystem suite

The suite (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/>) groups its
cases into modes, and a run picks one. This directory holds one committed pass
list per mode and served surface: the tests FerroTERM passes, one name per
line, sorted. CI runs the suite with the FHIR Validator's `txTests` command
(`scripts/checks/tx-ecosystem.sh`, the validator and the suite commit pinned
there) and fails when a listed test stops passing. A test that starts passing
is printed by the script; add it to the list in the same change that earns it.
`total.txt` records the number of cases the `general` mode runs; the script
fails when a run disagrees with it, and `scripts/checks/conformance-badges.sh`
turns the lists and the total into the README's badges at site build time.

## What is committed, per mode

Recorded on 2026-09-05 with validator 6.10.3 and suite
`eaec771d82fba4eac596c14963546f39b4ecffe7` (tests v1.9.3). Every mode run also
carries the two mode-independent `metadata` cases, so a mode's count is two
above the suite's own.

| mode | surface | pass list | passed of ran | needs | open failures |
|---|---|---|---|---|---|
| `general` | `/r4b` | `passing.txt` | 517 of 670 | nothing | #353 |
| `general` | `/r4` | `passing-r4.txt` | 518 of 670 | nothing | #353 |
| `general` | `/r5` | `passing-r5.txt` | 523 of 670 | nothing | #353 |
| `snomed` | `/r4b` | `passing-snomed.txt` | 1 of 170 | a SNOMED CT edition | #344, #352, #349 |
| `icd-11` | `/r4b` | `passing-icd-11.txt` | 44 of 52 | the three ICD-11 artifacts | #350, #349, #117 |
| `tx.fhir.org` | `/r4b` | `passing-tx.fhir.org.txt` | 29 of 227 | a LOINC release | #348, #305 |
| `tx.fhir.org` | `/r5` | `passing-r5-tx.fhir.org.txt` | 30 of 227 | a LOINC release | #348, #305 |
| `mimetypes` | `/r4b` | `passing-mimetypes.txt` | 23 of 37 | nothing | #346 |
| `omop` | `/r4b` | `passing-omop.txt` | 1 of 28 | an OMOP vocabulary the server does not load | #345 |

`general` is the only mode CI runs, on all three surfaces. The others need
licensed content or a code system the server does not serve, so they are run by
hand before a release and their lists are refreshed in the same change.

## What each mode needs to run

- **`general`** needs nothing: the runner supplies every code system and value
  set the cases use as `tx-resource` parameters.
- **`snomed`** needs a SNOMED CT artifact
  (`--mode snomed --index <edition>`). It scores 1 of 170 whatever edition you
  point it at, because every case pins the reference server's own edition
  (`http://snomed.info/xsct/31000003106/version/20250909`), which no release
  centre distributes. #344 holds the evidence.
- **`icd-11`** needs the three ICD-11 artifacts built from the WHO ICD-API
  local deployment (`--mode icd-11 --index <mms>:<icf>:<entity>`); the
  artifacts need the WHO container and its licence.
- **`tx.fhir.org`** needs a LOINC artifact (`--mode tx.fhir.org --index
  <loinc>`), since the release needs a LOINC account. The suite's own
  `tests/readme.md` says this mode is "for tests that are intended to be and
  written specifically for tx.fhir.org - internal QA. There is no need for
  other servers to pass these tests", so the LOINC subset is the part this
  project chases; 61 of its cases call a `ValueSet/$compare` operation no
  version's `OperationDefinition` declares, and answer 405. Four more send
  `_limit` on `$expand`, a name no `OperationDefinition` declares and the
  suite never documents, so they answer 400 and stay unpassed (#305).
- **`mimetypes`** needs nothing: BCP 13 is served from the vendored IANA media
  types registry.
- **`omop`** needs the OMOP standardized vocabularies, which this server does
  not load and which OHDSI distributes through Athena under per-vocabulary
  licences. The mode runs and scores 1 of 28. Its setup ValueSet is also
  missing the required `ValueSet.status`, so every case is refused before the
  vocabulary would be reached. #345 holds both findings.

## Known failures that cut across every mode

- **`$subsumes` never reaches the server.** Validator 6.10.3 throws
  `Unknown Operation subsumes` before it connects, which costs 98 cases across
  four modes. #346.
- **The `metadata` case fails on `/r4b`.** It asserts
  `CapabilityStatement.fhirVersion` is `4.0.1`; `/r4b` correctly reports
  `4.3.0`. It is on the `/r4` and `/r5` lists and off every `/r4b` one.

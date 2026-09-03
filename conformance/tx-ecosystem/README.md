# The HL7 terminology ecosystem suite

`passing.txt` lists the tests of the HL7 terminology ecosystem suite
(<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/>, `general` mode) that
FerroTERM passes on its R4B surface, one name per line, sorted. CI runs the
suite with the FHIR Validator's `txTests` command
(`scripts/checks/tx-ecosystem.sh`, the validator and the suite commit pinned
there) and fails when a listed test stops passing. A test that starts passing
is printed by the script; add it to the list in the same change that earns it.
The remaining failures and their adjudication are recorded on the tracker
(issue #89).

`passing-icd-11.txt` is the same list for the suite's `icd-11` mode, run by
hand over the three ICD-11 artifacts built from the WHO ICD-API local
deployment (`scripts/checks/tx-ecosystem.sh --mode icd-11 --index
<mms>:<icf>:<entity>`); the artifacts need the WHO container and its licence,
so this mode does not run in CI. The failures and their clusters are recorded
on issue #18.

`passing-tx.fhir.org.txt` is the list for the suite's `tx.fhir.org` mode,
which holds the LOINC cases; run by hand over a LOINC artifact
(`scripts/checks/tx-ecosystem.sh --mode tx.fhir.org --index <loinc>`), since
the release needs a LOINC account. The failures and their classes are recorded
on issue #13.

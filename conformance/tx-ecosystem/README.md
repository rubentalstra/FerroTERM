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

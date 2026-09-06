# Recorded capability documents

Four `TerminologyCapabilities` documents, one per FHIR root the server mounts.
Each is the body `GET /{version}/metadata?mode=terminology` answered, recorded
verbatim and pretty-printed. `src/fhir/terminology.rs` reads them in its unit
tests, so the viewer's capability reading is pinned against what the server
actually sends rather than against a shape someone remembered. They live here
rather than under `tests/`, which Cargo reserves for the integration-test
binary.

The server that answered them was loaded the way
`app/ferroterm-server/tests/it/fixture.rs` builds `start_with_every_loader`:
the `ferroterm-testkit` synthetic SNOMED edition, the LOINC and RxNorm
artifacts, the testkit `CodeSystem` resources, and the code systems the binary
always serves. Ten systems answer, which is what makes the documents worth
recording: the reader is exercised over several loaders, a version identifier
that is the empty string, versions with no designation language, and a system
that declares no hierarchy.

The content is synthetic. No SNOMED CT, LOINC, or RxNorm release content
appears here, and none ever may.

To re-record, drive that harness for each of `r4`, `r4b`, `r5`, and `r6` and
write the answered body to the matching file.

# Recorded capability documents

Eight documents, two per FHIR root the server mounts. Each
`terminology-capabilities-{version}.json` is the body
`GET /{version}/metadata?mode=terminology` answered, and each
`capability-statement-{version}.json` is the body `GET /{version}/metadata`
answered, both recorded verbatim and pretty-printed.
`src/fhir/terminology.rs` and `src/comparison.rs` read them in their unit
tests, so the viewer's capability reading is pinned against what the server
actually sends rather than against a shape someone remembered. They live here
rather than under `tests/`, which Cargo reserves for the integration-test
binary.

The capability statements are what the version comparison screen is built on:
which operations each root declares, at which levels, and what each says it
answers beyond its own release's definition. The differences between them are
the point, so a change to the render belongs in a re-recording.

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

To re-record, drive that harness for each of `r4`, `r4b`, `r5`, and `r6`, read
both `metadata` bodies, and write each to the matching file. A capability
statement carries a wall-clock `date`, so a recording always differs there.

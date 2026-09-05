# The differential check against Snowstorm

`requests.json` is the sample both servers answer: SNOMED CT identifiers, an
operation, and the projection to diff, and nothing else. It carries no term, no
definition, and no hierarchy, so it distributes no SNOMED CT content
(`.claude/rules/vendored-inputs.md`).

`scripts/checks/differential.sh` replays the sample against FerroTERM and
against Snowstorm over the same licensed edition and diffs one projection per
answer (`display`, `result`, `outcome`, `codes`, `targets`), because the two
servers return the same answer in different shapes. It refuses to compare two
servers that report different editions, so a difference is always about
behaviour. Where Snowstorm and the specification disagree the specification
wins, and the divergence is recorded on the tracker
(`.claude/rules/testing.md`).

Run it over a locally built index and a Snowstorm you already have:

```bash
cargo build --release -p ferroterm-server
scripts/checks/differential.sh --index artifacts/int --snowstorm http://127.0.0.1:8080/fhir
```

The harness never starts a Snowstorm: that deployment wants Elasticsearch and
16 to 32 GB of RAM.

`.github/workflows/differential.yml` runs the same script weekly and files an
issue on divergence. It skips itself until two repository secrets are set:
`SNOMED_RF2_URL`, one licensed RF2 release zip the runner can fetch, and
`SNOWSTORM_URL`, the FHIR base of a Snowstorm loaded with that same release.
Each run uploads a `differential-report` artifact holding every answer and the
divergences. Those runs are the recorded evidence behind the manual
differential criterion on issue #39; issues #15 and #19 name the Nictiz
Nationale Terminologieserver instead, and #247 runs this same harness against
it.

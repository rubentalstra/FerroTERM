# ferroterm-bench

The benchmark harness: one record per code system and run, from the
owner-licensed releases and artifacts under `data/` and `artifacts/`, never
committed. Hand-written; no FHIR or SNOMED specification governs a benchmark,
so every number carries its method.

- A record names the machine, the FerroTERM version, the code system release,
  and the method beside every figure; a figure without its conditions is not
  reported.
- Ingest is timed around `ferroterm-build` as a child process, its peak memory
  read from `/usr/bin/time`; the server is started as a child process and its
  resident memory sampled with `ps`; latency is measured over HTTP with a
  fixed request set, cold (the first request) and warm (percentiles over the
  rest).
- Reference-server comparisons run on the same machine over the same release
  or are recorded as not run; nothing is typed by hand.

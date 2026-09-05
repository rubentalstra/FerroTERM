# Benchmarks

Every figure FerroTERM quotes about speed or footprint comes from a record the
benchmark harness wrote, never from a number typed by hand. A record names the
machine, the FerroTERM version, the code system release, and the method beside
every figure, so you can rerun it and compare.

<!-- toc -->

## What a record holds

For one code system and one run, `ferroterm-bench` writes a JSON record with:

- the machine (operating system, architecture, CPU, memory) and the FerroTERM
  version;
- the code system, its version, its concept count, and the artifact's size on
  disk;
- the ingest: wall time around `ferroterm-build` over the release, and the
  build's peak resident memory (from `/usr/bin/time`), when the release is at
  hand;
- the time from starting the server until `/health` answers, and the server's
  resident memory after start and after the warm requests (`ps -o rss=`);
- per operation (`$lookup`, `$validate-code`, `$subsumes`, a small and a large
  `$expand`, and a designation search through `filter`): the first request
  cold, and the nearest-rank p50, p95, and p99 over the warm requests that
  follow, all as HTTP round trips from the same machine;
- the comparison field, which states a reference server's numbers taken on the
  same machine over the same release with its configuration, or says the
  comparison was not run.

## Reproduce a record

The harness runs over releases and artifacts you are licensed for; the
repository ships none. Build the artifacts as the [loading
page](../operate/loading-snomed.md) shows, put their paths in
`bench/systems.json`, and run:

```console
$ bench/run.sh
```

`bench/run.sh --skip-ingest` measures the artifacts as they are without
rebuilding them; `--only LOINC` restricts the run to systems whose name
contains the text. Records land under `bench/results/`, one file per system
and run, named by the system and the timestamp. Close other work on the
machine during a run; the latency of a request that shares a CPU with a
compiler is the compiler's number, not the server's.

## Run in a container

A record taken in the same container image is comparable across machines and
free of whatever else a workstation has installed. `bench/compose.yaml` builds
the server, the build tool, and the harness from the checkout on digest-pinned
Debian images, mounts `data/` and `artifacts/` read-only, and writes the records
to `bench/results/`:

```console
$ docker compose -f bench/compose.yaml build
$ docker compose -f bench/compose.yaml run --rm bench --skip-ingest --only LOINC
```

The record marks a container run (`machine.container`), and on macOS or Windows
the machine it names is the Docker virtual machine, with that machine's CPU
count and memory, so a container record from a laptop reads slower than a native
one on the same hardware. The README table says which kind it shows.

## Units, and what a record refuses

The records store raw numbers: milliseconds for latency, bytes for memory and
disk, seconds for ingest and time to ready. Everything rendered from them (the
console summary, the README table) uses the unit that fits the value: seconds,
milliseconds, or microseconds for time; GB, MB, or KB for bytes. A request that
answers anything other than a 2xx status fails the run for that system and no
record is written, so a record never holds the latency of an error response. A
cell reads `n/a` when the system defines no such operation, for instance a
code system with no whole-system value set to expand.

## The published figures

The README table, the figure tiles on the landing page, and the tables on the
site's [benchmarks page](https://ferroterm.eu/benchmarks.html) are rendered
from one committed record set, a directory under `bench/records/` named by the
date and the machine, by `scripts/checks/bench-table.sh render <target>`; CI
runs `check` and fails when any of the three and the records disagree. Every
record in a set comes from the same machine, FerroTERM version, and FHIR
version, or the renderer refuses the set. A new set is a new directory copied
from `bench/results/` by hand, so every published number names the run it came
from and links to its record.

## The current record set

<!-- bench-table:begin -->
| Code system | Release | Concepts | Build | Peak build memory | Index on disk | Resident | `$lookup` | `$validate-code` | `$subsumes` | `$expand` (small) | `$expand` (large) | Search | Snowstorm |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| [ICD-10-CM](https://github.com/rubentalstra/FerroTERM/blob/main/bench/records/2026-09-05-apple-m2/icd-10-cm-2026-09-05T22-33-18-666831Z.json) | 2026 | 98,827 | 1.58 s | 346 MB | 40 MB | 62 MB | 117 µs | 49 µs | 53 µs | n/a | n/a | n/a | not run |
| [ICD-10-NL](https://github.com/rubentalstra/FerroTERM/blob/main/bench/records/2026-09-05-apple-m2/icd-10-nl-2026-09-05T22-33-19-90564Z.json) | 2021 | 42,769 | 1.07 s | 235 MB | 20 MB | 36 MB | 92 µs | 60 µs | 55 µs | n/a | n/a | n/a | not run |
| [ICD-11 MMS](https://github.com/rubentalstra/FerroTERM/blob/main/bench/records/2026-09-05-apple-m2/icd-11-mms-2026-09-05T22-33-25-155842Z.json) | 2026-01 | 37,211 | n/a | n/a | 34 MB | 74 MB | 219 µs | 53 µs | n/a | 67 µs | 73 µs | 78 µs | not run |
| [LOINC](https://github.com/rubentalstra/FerroTERM/blob/main/bench/records/2026-09-05-apple-m2/loinc-2026-09-05T22-33-16-915837Z.json) | 2.83 | 257,266 | 10.66 s | 2.1 GB | 262 MB | 170 MB | 175 µs | 62 µs | n/a | 59 µs | 8.02 ms | 244 µs | not run |
| [RxNorm (prescribable subset)](https://github.com/rubentalstra/FerroTERM/blob/main/bench/records/2026-09-05-apple-m2/rxnorm-prescribable-subset-2026-09-05T22-33-24-820155Z.json) | 09082026 | 81,468 | 4.45 s | 637 MB | 72 MB | 119 MB | 987 µs | 114 µs | n/a | n/a | n/a | n/a | not run |
| [SNOMED CT (International edition)](https://github.com/rubentalstra/FerroTERM/blob/main/bench/records/2026-09-05-apple-m2/snomed-ct-international-edition-2026-09-05T22-33-04-380763Z.json) | 20260901 | 535,502 | 18.52 s | 2.52 GB | 626 MB | 703 MB | 527 µs | 89 µs | 66 µs | 175 µs | 1.85 ms | 304 µs | not run |
| [SNOMED CT (Netherlands edition)](https://github.com/rubentalstra/FerroTERM/blob/main/bench/records/2026-09-05-apple-m2/snomed-ct-netherlands-edition-2026-09-05T22-32-44-742097Z.json) | 20260630 | 548,949 | 30.19 s | 3.43 GB | 864 MB | 888 MB | 534 µs | 87 µs | 87 µs | 235 µs | 2.44 ms | 751 µs | not run |

Warm p50 over 200 HTTP round trips on one machine (Apple M2, 17.18 GB, macos/aarch64), FerroTERM 0.1.0 serving FHIR R4B, taken 2026-09-05. The records are under `bench/records/`; the [benchmarks page](https://ferroterm.eu/benchmarks.html) has the method, the cold and tail latencies, and how to reproduce a record.
<!-- bench-table:end -->

## Reading the numbers

A cold request is the first of its kind on a freshly started server, before
`redb`'s page cache and the query caches hold anything for that path; the warm
percentiles are what a busy server answers. The resident memory is what the
server has read in: the index is loaded at startup, which is why the figure
after start and the figure after the warm requests are close. The design target
for point reads is under a millisecond in the release profile on the Dutch
edition; a millisecond-scale figure is a measurement to improve, never a result
to call fine.

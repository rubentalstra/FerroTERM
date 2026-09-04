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
| [ICD-10-CM](https://github.com/rubentalstra/FerroTERM/blob/main/bench/records/2026-09-04-docker-apple-m2/icd-10-cm-2026-09-04T09-16-57-858505552Z.json) | 2026 | 98,827 | 1.7 s | 319 MB | 40 MB | 45 MB | 104 µs | 103 µs | 111 µs | n/a | n/a | n/a | not run |
| [ICD-11 MMS](https://github.com/rubentalstra/FerroTERM/blob/main/bench/records/2026-09-04-docker-apple-m2/icd-11-mms-2026-09-04T09-17-03-558396221Z.json) | 2026-01 | 37,211 | n/a | n/a | 34 MB | 60 MB | 263 µs | 115 µs | n/a | 126 µs | 125 µs | 139 µs | not run |
| [LOINC](https://github.com/rubentalstra/FerroTERM/blob/main/bench/records/2026-09-04-docker-apple-m2/loinc-2026-09-04T09-16-55-946117093Z.json) | 2.83 | 257,266 | 12.89 s | 2.27 GB | 260 MB | 124 MB | 265 µs | 116 µs | n/a | 127 µs | 6.46 ms | 1.45 ms | not run |
| [RxNorm (prescribable subset)](https://github.com/rubentalstra/FerroTERM/blob/main/bench/records/2026-09-04-docker-apple-m2/rxnorm-prescribable-subset-2026-09-04T09-17-03-166797554Z.json) | 09082026 | 81,468 | 4.49 s | 568 MB | 72 MB | 86 MB | 1.41 ms | 134 µs | n/a | n/a | n/a | n/a | not run |
| [SNOMED CT (International edition)](https://github.com/rubentalstra/FerroTERM/blob/main/bench/records/2026-09-04-docker-apple-m2/snomed-ct-international-edition-2026-09-04T09-16-41-025864294Z.json) | 20260901 | 535,502 | 36.81 s | 2.49 GB | 486 MB | 402 MB | 915 µs | 116 µs | 111 µs | 526 µs | 3.9 ms | 357 µs | not run |
| [SNOMED CT (Netherlands edition)](https://github.com/rubentalstra/FerroTERM/blob/main/bench/records/2026-09-04-docker-apple-m2/snomed-ct-netherlands-edition-2026-09-04T09-16-02-189844762Z.json) | 20260630 | 548,949 | 62.92 s | 3.41 GB | 668 MB | 475 MB | 934 µs | 118 µs | 112 µs | 838 µs | 6.91 ms | 869 µs | not run |

Warm p50 over 200 HTTP round trips on one machine, inside a Docker container (Apple aarch64 (implementer 0x61, part 0x000), 8.22 GB, linux/aarch64), FerroTERM 0.0.9 serving FHIR R4B, taken 2026-09-04. The records are under `bench/records/`; the [benchmarks page](https://ferroterm.eu/benchmarks.html) has the method, the cold and tail latencies, and how to reproduce a record.
<!-- bench-table:end -->

## Reading the numbers

A cold request includes the first page faults of the memory-mapped index for
that path; the warm percentiles are what a busy server answers. The resident
memory is mostly the memory-mapped files the kernel has paged in; it moves with
the page cache and shrinks under pressure. The design target for point reads
is under a millisecond in the release profile on the Dutch edition; a
millisecond-scale figure is a measurement to improve, never a result to call
fine.

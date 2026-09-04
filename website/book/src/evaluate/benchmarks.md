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

## The README table

The README's speed and footprint table is rendered from a committed record set,
one directory under `bench/records/` named by the date and the machine, by
`scripts/checks/bench-table.sh render`; CI runs `check` and fails when the table
and the records disagree. Every record in a set comes from the same machine,
FerroTERM version, and FHIR version, or the renderer refuses the set. A new set
is a new directory copied from `bench/results/` by hand, so the numbers on the
README always name the run they came from.

## Reading the numbers

A cold request includes the first page faults of the memory-mapped index for
that path; the warm percentiles are what a busy server answers. The resident
memory is mostly the memory-mapped files the kernel has paged in; it moves with
the page cache and shrinks under pressure. The design target for point reads
is under a millisecond in the release profile on the Dutch edition; a
millisecond-scale figure is a measurement to improve, never a result to call
fine.

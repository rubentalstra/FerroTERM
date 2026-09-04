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

## Reading the numbers

A cold request includes the first page faults of the memory-mapped index for
that path; the warm percentiles are what a busy server answers. The resident
memory is mostly the memory-mapped files the kernel has paged in; it moves with
the page cache and shrinks under pressure. The design target for point reads
is under a millisecond in the release profile on the Dutch edition; a
millisecond-scale figure is a measurement to improve, never a result to call
fine.

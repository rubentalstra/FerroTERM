# Hardware sizing

FerroTERM is built to run an edition on ordinary hardware. This page says
where the memory goes and how to size a machine; the measurements themselves
(build time and peak build memory per release, the index on disk, the server's
resident memory, the latency of every operation) are on the
[benchmarks page](../evaluate/benchmarks.md), rendered from the committed
records so no figure on this site is typed by hand. Your numbers vary with the
edition and the machine; the shape below holds.

<!-- toc -->

## What the records show

The resident figure is mostly the memory-mapped files the kernel has paged in;
it moves with the page cache and shrinks under pressure, so a server that shows
several hundred MB resident after warm-up is not holding that much heap. The
build is the expensive step: it holds a whole release in memory while it
computes the transitive closure, so peak build memory is several times the
size of the finished index and is the figure to size a build machine by. The
design target for point reads is under 1 ms measured over HTTP on the same
machine; a millisecond-scale figure in a record is tracked as a performance
issue, not called done.

## Where the memory goes

| Structure | Held | Rough size for the Dutch edition |
|---|---|---|
| Transitive closure, both directions, as roaring bitmaps | resident after the first touch | the larger part of `hierarchy.bin` |
| CSR is-a adjacency | resident | tens of MB |
| `fst` word dictionary and postings | memory-mapped | `text.bin` |
| Concepts, designations, properties | memory-mapped `redb`, paged on demand | most of `store.redb` |

Both directions of the closure are stored on purpose: subsumption needs one
direction, and a descendant set is returned directly from the other. Roaring
compresses SNOMED-shaped sets heavily, which is why the closure of half a
million concepts fits in hundreds of megabytes rather than the gigabytes a plain
bitset would need.

## Disk and CPU

Size the disk for the indexes you load; the benchmarks page gives each index's size. The
offline build is the CPU-heavy step, once per release, in `ferroterm-build`.
Serving is a point read or a bitmap operation, so a modest CPU handles it: a
2 to 4 GB box with two cores serves an edition beside other services. A paged
`$expand` is cut from the selection before any concept is read, so the
concepts a page reads are the concepts it returns, not the whole selection;
an unpaged expansion beyond 1,000 members is refused with `too-costly`.
Finding the page still costs something on a large selection: the committed
records put a hundred members of a SNOMED subtree at 253 microseconds and a
thousand at 2.5 milliseconds on the Netherlands edition.

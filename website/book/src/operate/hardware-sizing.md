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

The resident figure is what the server has read in and holds. The index is read
at startup, not paged in as requests arrive, so the figure is at its full size
by the time `/health` answers 200; the records carry the resident memory at
that point and again after the warm requests, and the two are close. Size the
machine for that figure. The build is the expensive step: it holds a whole
release in memory while it computes the transitive closure, so peak build
memory is several times the size of the finished index and is the figure to
size a build machine by. The
design target for point reads is under 1 ms measured over HTTP on the same
machine; a millisecond-scale figure in a record is tracked as a performance
issue, not called done.

## Where the memory goes

| Structure | Held | Rough size for the Dutch edition |
|---|---|---|
| Transitive closure, both directions, as roaring bitmaps | resident, read at startup | the larger part of `hierarchy.bin` |
| CSR is-a adjacency | resident, read at startup | tens of MB |
| `fst` word dictionary and roaring postings | resident, read at startup | `text.bin` |
| Concepts and displays, dense columns addressed by ordinal | resident, read when the store opens | part of `store.redb` |
| Designations, acceptability, properties | left in the file, point-read through `redb`'s page cache | most of `store.redb` |

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
`$expand` is cut from the bitmaps before any concept is read, so a page of
ten out of 133,736 descendants costs the same as a page of ten out of ten;
an unpaged expansion beyond 1,000 members is refused with `too-costly`.

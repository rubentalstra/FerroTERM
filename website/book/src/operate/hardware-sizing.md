# Hardware sizing

FerroTERM is built to run an edition on ordinary hardware. This page gives
measurements of v0.0.7 and where the memory goes.

> [!NOTE]
> The figures were taken on a laptop over the licensed Dutch SNOMED CT edition
> of June 2026 (548,949 concepts, active and inactive), with the public
> ICD-10-CM FY2026 release and the RxNorm prescribable subset loaded beside it.
> Your numbers vary with the edition and the machine; the shape holds.

<!-- toc -->

## Measured

| What | Figure |
|---|---|
| Build the Dutch edition from the release zip | 49 s |
| The Dutch edition on disk | 591 MB (`store.redb`, `hierarchy.bin`, `text.bin`) |
| ICD-10-CM FY2026 (98,827 codes) | 8 s to build, 38 MB |
| RxNorm prescribable subset (81,468 concepts, 1.13 million edges) | 11 s to build, 68 MB |
| Server start with the three indexes | 0.5 s |
| Resident memory after start, all three | 578 MB |
| Resident memory after 200 lookups and two expansions | 423 MB |
| `$lookup` of a SNOMED CT concept, end to end with `curl` | about 2.5 ms |
| `$lookup` of an ICD-10-CM code | about 0.6 ms |

The resident figure is mostly the memory-mapped files the kernel has paged in;
it moves with the page cache and shrinks under pressure. The design target for
point reads is under 1 ms; the millisecond-scale figures above are measured
over HTTP with `curl` on the same machine and are tracked as a performance
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

Size the disk for the indexes you load; the table above gives the shape. The
offline build is the CPU-heavy step, once per release, in `ferroterm-build`.
Serving is a point read or a bitmap operation, so a modest CPU handles it: a
2 to 4 GB box with two cores serves an edition beside other services. A paged
`$expand` is cut from the bitmaps before any concept is read, so a page of
ten out of 133,736 descendants costs the same as a page of ten out of ten;
an unpaged expansion beyond 1,000 members is refused with `too-costly`.

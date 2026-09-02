# Hardware sizing

FerroTERM targets a small footprint so you can run the full International edition on
ordinary hardware. This page gives the design targets and where the memory goes.

> [!NOTE]
> These are design targets grounded in the reference servers and in roaring-bitmap
> compression behaviour, not measurements of a shipped build. See
> [`docs/architecture.md`](https://github.com/rubentalstra/FerroTERM/blob/main/docs/architecture.md)
> for the reasoning.

<!-- toc -->

## The target

The design target is to serve the full SNOMED CT International edition in a few
hundred megabytes of resident memory, so the server fits on a 2 to 4 GB box with
room to spare. For comparison, SNOMED International's Snowstorm Lite runs the same
edition in about 500 MB, and the Java-plus-Elasticsearch Snowstorm wants 16 to
32 GB.

## Where the memory goes

At startup the server loads the reachability closure into resident memory and
leaves the rest on the memory-mapped index. The expected split:

| Structure | Resident | Rough size |
|---|---|---|
| Transitive closure (ancestor and descendant bitmaps) | yes | 100 to 300 MB |
| CSR adjacency (is-a and per-attribute) | yes | tens of MB |
| `fst` text dictionary | yes | tens of MB |
| Columnar concept and description store | memory-mapped, paged on demand | on disk |

Both directions of the closure are stored on purpose: subsumption needs one
direction, and ECL returns each set directly, so keeping both trades roughly 2x
the closure space for direct answers. Roaring compresses SNOMED-shaped sets
heavily, which is why the resident closure lands in the hundreds of megabytes
rather than the gigabytes a naive bitset would need.

## Disk and CPU

The index on disk is the memory-mapped `redb` file the build tool writes. Size it
for the edition you load. The offline build is the CPU-heavy step, and it runs
once per release in the build tool, not on the server. Serving is a point read or
a bitmap operation, so a modest CPU handles it. A heavy `$expand` that
materializes a large set, and any cold read that page-faults from disk, run on a
blocking pool so they never stall the request runtime.

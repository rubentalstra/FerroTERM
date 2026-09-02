# Configuration

The server reads its configuration from environment variables at start. There
is no configuration file and no command-line flag today; a container sets the
variables, a host sets them in the service unit.

<!-- toc -->

## The variables

| Variable | Meaning | Default |
|---|---|---|
| `FERROTERM_INDEX` | The artifact directories to serve, one per code system version, separated by the platform's path separator (`:` on Linux). Each holds the `store.redb` and `manifest.json` that `ferroterm-build` wrote. The server opens them read-only and refuses to start when one is missing, damaged, or duplicates another's system version. | none (the server starts with no code systems) |
| `FERROTERM_LISTEN` | The socket address to bind. | `127.0.0.1:8080` (the container image sets `0.0.0.0:8080`) |
| `FERROTERM_DEFAULT_LANGUAGE` | The BCP 47 language used for `display` when a request names none. | `en` |
| `RUST_LOG` | The `tracing` filter for the log output. | the `tracing-subscriber` default |

## What a code system version is served as

Each loaded artifact becomes one `CodeSystem` instance whose id is derived
from its version URI (for a SNOMED CT edition,
`snomed.info-sct-<module>-version-<YYYYMMDD>`), so an instance-level operation
is `[base]/r4b/CodeSystem/<id>/$validate-code`. The id is stable for a given
version and is listed at startup and in `GET /r4b/metadata?mode=terminology`.

## Planned settings

The served FHIR versions (R4B today; R5, R4, and R6 follow) and the
`ValueSet/$expand` page limits get their variables when those surfaces land;
they are recorded here when they do.

## What you do not configure

You do not configure a database connection, a search cluster, or a JVM heap.
FerroTERM has none of these. The one input the server needs is the built index, and
you build that offline with the release-loading tool described in
[Loading a SNOMED CT edition](loading-snomed.md).

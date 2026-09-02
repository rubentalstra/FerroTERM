# Configuration

The server reads its configuration from environment variables at start. There
is no configuration file and no command-line flag today; a container sets the
variables, a host sets them in the service unit.

<!-- toc -->

## The variables

| Variable | Meaning | Default |
|---|---|---|
| `FERROTERM_INDEX` | The artifact directories to serve, one per code system version, separated by the platform's path separator (`:` on Linux). Each holds the `store.redb` and `manifest.json` that `ferroterm-build` wrote. The server opens them read-only and refuses to start when one is missing, damaged, or duplicates another's system version. | none (the server starts with no code systems) |
| `FERROTERM_CODESYSTEMS` | Directories of FHIR `CodeSystem` resources to serve, separated by the platform's path separator: a FHIR package's `package/` directory (HL7 Terminology, for example) or a directory of `CodeSystem` JSON files. Files are read in the FHIR version the directory's `package.json` declares, or as R4B when there is none. A resource with `content = supplement` is applied to the system it `supplements` and is not served as an instance; the server refuses to start when that system is not loaded. | none |
| `FERROTERM_LISTEN` | The socket address to bind. | `127.0.0.1:8080` (the container image sets `0.0.0.0:8080`) |
| `FERROTERM_DEFAULT_LANGUAGE` | The BCP 47 language used for `display` when a request names none. | `en` |
| `FERROTERM_LOG_FORMAT` | `auto`, `json`, or `pretty`. `json` writes one object per line (`timestamp`, `level`, `message`, and the fields) for a log pipeline; `pretty` writes aligned human lines with colour; `auto` picks `pretty` when stdout is a terminal and `json` otherwise. The startup banner prints only with `pretty`. | `auto` |
| `RUST_LOG` | The `tracing` filter. | `info,hyper=warn,tower=warn,h2=warn` |

## What the server logs

At start: the banner (pretty only), one `ferroterm starting` line with the
version and the count of code systems, one `serving code system` line per
loaded version (`id`, `system`, `version`, `concepts`, `languages`, `path`),
and one `listening` line with the address and the base path. Per request: one
`request` line with `method`, `route`, `status`, `latency_ms`, and the
`system`, `url`, `version`, `code`, `codeA`, `codeB` query parameters the
request named (`named`); bodies and free-text parameters are never logged. A
client error logs at `warn`, a server error at `error`. On `SIGTERM` or
`SIGINT`: the signal, then `ferroterm stopped`. A refused start is one
`cannot start` line with the reason.

## What a code system version is served as

Each loaded code system version becomes one `CodeSystem` instance. The id is
the version URI when that URI carries the system (a SNOMED CT edition is
`snomed.info-sct-<module>-version-<YYYYMMDD>`), otherwise the system URL and
the version (`terminology.hl7.org-CodeSystem-v2-0001-2.0.0`), reduced to the
FHIR id alphabet. An instance-level operation is
`[base]/r4b/CodeSystem/<id>/$validate-code`. The id is stable for a given
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

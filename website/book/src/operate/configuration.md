# Configuration

The server reads its configuration from environment variables at start. There
is no configuration file and no command-line flag; a container sets the
variables, a host sets them in the service unit. `ValueSet/$expand` returns
at most 1,000 members without `count` and asks for paging beyond that
(`too-costly`); the limit is not configurable.

<!-- toc -->

## The variables

| Variable | Meaning | Default |
|---|---|---|
| `FERROTERM_INDEX` | The artifact directories to serve, one per code system version (a SNOMED CT edition, a LOINC release, an ICD-10 classification, an RxNorm release, or one of the three ICD-11 code systems; the manifest says which), separated by the platform's path separator (`:` on Linux). Each holds the `store.redb`, `hierarchy.bin`, `text.bin`, and `manifest.json` that `ferroterm-build` wrote. The server opens them read-only and refuses to start when one is missing, damaged, or duplicates another's system version. | none (the server starts with no code systems) |
| `FERROTERM_CODESYSTEMS` | Directories of FHIR `CodeSystem` resources to serve, separated by the platform's path separator: a FHIR package's `package/` directory (HL7 Terminology, for example) or a directory of `CodeSystem` JSON files. Files are read in the FHIR version the directory's `package.json` declares, or as R4B when there is none. A resource with `content = supplement` is applied to the system it `supplements` and is not served as an instance; the server refuses to start when that system is not loaded. `ValueSet` resources in the same directories are served by `url` and `version` for `ValueSet/$expand` and `ValueSet/$validate-code` (without a version, the greatest version answers). | none |
| `FERROTERM_RESOURCES` | The database file holding the `CodeSystem`, `ValueSet`, and `ConceptMap` resources clients write through the REST API, and the closure tables `$closure` maintains. The server creates the file when it does not exist, serves every resource in it exactly as it serves one from `FERROTERM_CODESYSTEMS`, and loads them again after a restart. A deployment that names no file refuses every write with a `422` and declares no write interaction in its capability statement. | none (the server persists nothing) |
| `FERROTERM_LISTEN` | The socket address to bind. | `127.0.0.1:8080` (the container image sets `0.0.0.0:8080`) |
| `FERROTERM_ADMIN_LISTEN` | The socket address of the admin listener, which serves `POST /reload` and nothing else (see [Reloading the served set](#reloading-the-served-set)). It authenticates nobody, so bind it to an address only your operators reach. Unset, the server binds no second listener and `SIGHUP` is the only reload trigger. | none (no admin listener) |
| `FERROTERM_BASE_URL` | The URL clients reach this server at, without a version prefix and without a trailing slash (`https://tx.example.org`). Each version states it as `implementation.url` in its capability statements, so a client behind a reverse proxy learns the address it should use rather than the socket the process bound. A deployment that names none states no URL. | none |
| `FERROTERM_SECURITY_SERVICE` | The authentication in front of the server, as comma-separated codes of the FHIR `restful-security-service` value set (`OAuth`, `SMART-on-FHIR`, `Basic`, `Certificates`, `Kerberos`, `NTLM`). The server validates nothing itself; the codes declare what a proxy enforces, in `CapabilityStatement.rest.security.service`. | none |
| `FERROTERM_UI` | Whether the server serves the viewer at `/ui` and redirects `/` onto it. Reads `on` or `off` (`true`/`false`, `1`/`0`, `yes`/`no`). Off, the `/ui` routes are absent and both `/` and `/ui` answer the `not-found` `OperationOutcome` every unknown path answers, for a deployment that wants an API-only surface. A build that carries no viewer bundle serves no `/ui` route whatever this is set to, and says so at start. | `on` |
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
`cannot start` line with the reason. On a reload: `the served set was
reloaded` with the count of code systems, or `the served set was not
reloaded` at `error` level with the reason.

## Reloading the served set

The server reads the directories again and swaps the whole served set on two
triggers:

- `SIGHUP` to the process (`kill -HUP <pid>`, `docker kill --signal=HUP <name>`).
- `POST /reload` on the admin listener, when `FERROTERM_ADMIN_LISTEN` names an
  address.

```console
$ curl -sS -X POST http://127.0.0.1:8081/reload
{"outcome":"ok","systems":[{"id":"snomed.info-sct-…","system":"http://snomed.info/sct","version":"…"}]}
```

A reload opens every artifact in `FERROTERM_INDEX` and every resource in
`FERROTERM_CODESYSTEMS` again, read-only, builds a new registry, and exchanges
a pointer. A request already in flight finishes on the set it started on, and
that set is dropped when the last request holding it ends, so the memory of the
swapped system is resident twice for a few seconds. `FERROTERM_RESOURCES` is
live already and is carried over untouched: the resources clients wrote stay
written, and the new set serves them over the new artifacts.

An artifact is replaced by building it in a new directory and renaming that
directory over the configured path, which is one atomic step and leaves the
old files in place for the handles still reading them. Writing into the
directory the running server is reading corrupts what it reads. A directory in
`FERROTERM_CODESYSTEMS` is different: add or remove a resource file in it, and
the reload picks up the change.

Adding or removing a whole artifact path means changing `FERROTERM_INDEX`,
which the process reads once at start, so that still takes a restart.

The admin listener carries `POST /reload` and nothing else, answers `404` to
anything else, and authenticates nobody. Bind it to a loopback or internal
address (`127.0.0.1:8081`), keep it off the address your clients reach, and
leave it out of the reverse proxy that publishes the FHIR surface (see
[Behind a reverse proxy](reverse-proxy.md)). The FHIR listener never serves
`/reload`: that path is the `not-found` `OperationOutcome` any unknown path
answers.

## A refused artifact stops the start

An artifact the server cannot open stops the whole start. Nine good artifacts
and one damaged one serve nothing, and the `cannot start` line names the
directory and the reason.

That is deliberate. A server that dropped the damaged one and served the other
nine would answer every code of the tenth system with "not found", and a caller
cannot tell that from "this code does not exist". A terminology answer that is
wrong and confident is worse for a clinical system downstream than a server
that did not start, so the refusal is loud and total.

The practical consequence for an operator is that a rebuilt artifact is
swapped in before a restart, not after one. The practical consequence for a
benchmark or a conformance run is that it fails at once, naming the artifact,
rather than reporting a figure taken over a system nothing served.

A reload holds the same bar without the downtime. One artifact that does not
open refuses the whole swap: the server keeps serving the set it already had,
logs `the served set was not reloaded` with the reason, counts
`ferroterm_reloads_total{outcome="failed"}`, and answers the admin request
with a `500` naming the artifact. A half-applied release never reaches a
client.

## The registry systems

Four systems ship with the server and are served without configuration: BCP
47 language tags (`urn:ietf:bcp:47`), BCP 13 media types (`urn:ietf:bcp:13`),
UCUM units (`http://unitsofmeasure.org`), and ISO 3166-1 country codes
(`urn:iso:std:iso:3166`). The first three are grammars over their registries
and cannot be expanded; the fourth is a table from Unicode CLDR. Their data
is vendored in the binary.

## What a code system version is served as

Each loaded code system version becomes one `CodeSystem` instance. The id is
the version URI when that URI carries the system (a SNOMED CT edition is
`snomed.info-sct-<module>-version-<YYYYMMDD>`), otherwise the system URL and
the version (`terminology.hl7.org-CodeSystem-v2-0001-2.0.0`), reduced to the
FHIR id alphabet. The instance reads at `[base]/r4b/CodeSystem/<id>`, is found
by `[base]/r4b/CodeSystem?url=…&version=…`, and takes an instance-level
operation at `[base]/r4b/CodeSystem/<id>/$validate-code`. The id is stable for
a given version and is listed at startup and in
`GET /r4b/metadata?mode=terminology`.

A `ValueSet` or `ConceptMap` in a `FERROTERM_CODESYSTEMS` directory becomes an
instance the same way, keyed by its `url` and `version`. It reads at
`[base]/r4b/ValueSet/<id>` or `[base]/r4b/ConceptMap/<id>`, is found by
`[base]/r4b/ValueSet?url=…&version=…` or the matching `ConceptMap` search, and
carries that id as its `Resource.id` in both answers, so a client can read what
a search returned. Every served version answers both.

## What you do not configure

You do not configure a database connection, a search cluster, or a JVM heap.
FerroTERM has none of these. The inputs the server needs are the built indexes and any
FHIR resource directories, and you build the indexes offline with
`ferroterm-build` as described in [Loading code systems](loading-snomed.md).

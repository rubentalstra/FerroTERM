# Syncing from a national terminology service

A national terminology service publishes its releases in an Atom syndication
feed: one entry per downloadable item, with the canonical identifier, the
version, the date, and a digest. The FerroTERM sync service reads such a feed
on a schedule, takes what you subscribed to, builds or stages it, and asks the
server to reload. It runs beside the server and never inside it, and it moves
content in one direction only.

## What one run does

1. It reads each configured feed and compares what it offers with what you
   already serve: the releases under your index root, the resources in your
   managed directory, and the ledger of what the service itself put there.
2. It takes the entries you subscribed to that you do not hold yet, streams
   each one to the staging directory, and refuses any whose digest does not
   match what the feed advertised.
3. An RF2 archive goes through `ferroterm-build` into a staging directory and
   is renamed into the index root as a new release beside the previous one. A
   FHIR resource is written as a file in the managed resource directory,
   corrected first when its service needs a correction.
4. It asks the server to reload, then prunes the releases retention no longer
   keeps and tells the server about that too.
5. It writes one JSON record of everything above and posts a summary to your
   webhook, whether the run worked or not.

A run that fails leaves the served set exactly as it was. A build that fails
never reaches the index root, and a reload the server refuses is rolled back,
including a resource file the run had replaced. Nothing is deleted except by
retention, and the server's own write store (`FERROTERM_RESOURCES`) is never
touched.

## Running it

The service ships as its own image, carrying `ferroterm-sync` and
`ferroterm-build`, and the Compose overlay runs it beside the server over one
volume:

```bash
FERROTERM_SYNC_DATA=/srv/ferroterm FERROTERM_SYNC_CONFIG=/srv/sync.toml \
  docker compose -f compose.yaml -f compose.sync.yaml up
```

One directory on one filesystem holds `index/`, `codesystems/`, `staging/`,
`records/`, and `state/`. That is not a convenience: a run stages a release and
then renames it into the index root, and a rename works only inside one
filesystem. The server mounts `index/` and `codesystems/` read-only; the sync
mounts the whole tree and writes to it as uid 65532.

The overlay also sets `FERROTERM_ADMIN_LISTEN` on the server, because
`POST /reload` on that listener is how the sync asks for the new release to be
served.

`ferroterm-sync run-once` performs a single run and exits, which is what a
first run and a cron-style deployment use.

## Configuration

One TOML file, `/etc/ferroterm/sync.toml` unless `--config` says otherwise.
Every key is optional and falls back to the value below; a key the service does
not know is refused at start-up rather than ignored.

| Key | Default | What it names |
|---|---|---|
| `listen` | `127.0.0.1:8181` | the service's own admin listener |
| `server_admin_url` | `http://127.0.0.1:8081` | the server's admin listener, which serves `POST /reload` |
| `index_root` | `/data/index` | the index root the server reads |
| `resources` | `/data/codesystems` | the managed resource directory the server reads |
| `staging` | `/data/staging` | where a run downloads and builds |
| `records` | `/data/records` | one JSON record per run |
| `state` | `/data/state` | what the service remembers between runs |
| `retention` | `2` | release directories kept per code system |
| `activation` | `auto` | `auto` or `manual` |
| `webhook_url` | none | where one JSON summary per run is posted |
| `timezone` | `UTC` | the zone a daily schedule is read in |
| `build_command` | `ferroterm-build` | the offline build, on `PATH` or by path |

A `[[source]]` entry names the add-on that reads a service and carries that
add-on's own block:

```toml
server_admin_url = "http://ferroterm:8081"
index_root = "/data/index"
resources = "/data/codesystems"
staging = "/data/staging"
records = "/data/records"
state = "/data/state"
retention = 2
activation = "auto"
timezone = "Europe/Amsterdam"

[schedule]
at = "03:00"

[[source]]
kind = "nts"

[source.config]
systems = ["http://snomed.info/sct/11000146104", "http://loinc.org"]
```

The file carries no credential. A source add-on reads its credentials from a
file or from the environment, so this file is safe to commit and safe to show.

## The schedule

Two shapes, and no cron syntax:

- `every = "24h"` measures from the end of the last run.
- `at = "03:00"` is a time of day in `timezone`.

With both, the earlier due time wins. With neither, the service runs only when
you ask it to. The end of the last run is written to the state directory, so a
restart between two runs does not replay the run that already happened: a
service that ran at 03:00 and restarts at 04:00 is next due at 03:00 the
following day. A window the service was down for is due at once.

A run also starts on `POST /run` on the admin listener, and as
`ferroterm-sync run-once`.

## Activation

In `auto` the run renames what it built into the index root and asks the server
to reload, in the same run. In `manual` the run stops after staging: nothing is
served, the run record says how many items wait, and `POST /activate` puts them
in front of the server when you are ready. What is staged survives a restart,
and a later run does not build it again.

## Retention

After a successful reload the service keeps the newest `retention` release
directories per code system and removes the rest, then asks the server to
reload once more so the removed release leaves the served set. A release that
is still there is what you roll back to, so keep at least two. A retention of
`0` removes nothing. Each run record says what was removed, how many bytes it
held, and how many bytes the index root uses afterwards.

Retention judges only directories that carry a `manifest.json`, so a build in
progress is never touched, and it groups them by the code system edition the
manifest names.

## The admin listener

The service binds `listen` and serves six routes:

| Route | What it does |
|---|---|
| `POST /run` | performs one run and answers its summary when the run has finished |
| `POST /activate` | puts what is staged in front of the server |
| `GET /runs` | the runs this service performed, newest first |
| `GET /runs/{id}` | one whole run record |
| `GET /metrics` | the Prometheus exposition |
| `GET /health` | `200` while the process is up |

It authenticates nobody, exactly like the server's admin listener. Keep it on
an internal network, and never publish it. The Compose overlay leaves both
listeners unpublished, reachable only inside the compose network.

`/metrics` carries `ferroterm_sync_runs_total{outcome="ok"|"failed"}`,
`ferroterm_sync_last_run_timestamp_seconds`,
`ferroterm_sync_entries_taken_total`, `ferroterm_sync_bytes_staged_total`, and
`ferroterm_sync_index_bytes`.

## What a run record contains

One JSON file per run under `records`, named by the run identifier, which sorts
by time. It carries:

- the trigger, the start and end, the duration, and the outcome;
- per source: the feed address, how many entries it offered, what was taken
  (with the bytes, the download time, the build time, and the corrections that
  were applied), and what was left behind with the reason for each one, such as
  a system you did not subscribe to, a release you already serve, a delta
  rather than a snapshot, or Ontoserver's binary index;
- the activation: the mode, what reached the server, what is still staged, what
  the server answered to each reload, and whether the run was rolled back;
- what retention removed and how many bytes the index root uses;
- every error, in the order it happened.

The same summary is what the webhook receives: one POST per run end, on success
and on failure. A delivery that fails is logged and never fails the run.

## NTS (Nationale Terminologie Server)

The [NTS](https://www.nictiz.nl/publicaties/nationale-terminologie-server-handleiding-voor-nieuwe-gebruikers/)
is the Dutch national terminology service, run by Nictiz. It serves FHIR R4 and
publishes its content at
`https://terminologieserver.nl/synd/syndication.xml`. The feed is challenged:
the listing itself answers `401` with a bearer challenge, so the add-on
authenticates before it can even see what is on offer.

### The account and the licences

You need a personal account on the NTS. Nictiz issues it after you accept its
terms, and each licensed system is unlocked per account: SNOMED CT (the Dutch
edition), LOINC, the Nederlandse Labcodeset, UCUM, ICD-10, the NHG tables, and
the zib value sets. An account without a licence for a system sees no entries
for it, so check the licences you hold before you subscribe to a canonical.

The content stays licensed by its publisher. FerroTERM distributes none of it,
and a deployment brings what it is licensed for.

### Credentials

The add-on reads its credentials from a file or from the environment, never
from the configuration body, so a configuration file is safe to commit and to
show in a run record.

| Variable | What it holds |
|---|---|
| `FERROTERM_NTS_CLIENT_ID` | The OAuth 2 client, `cli_client` by default |
| `FERROTERM_NTS_CLIENT_SECRET` | A client secret, when your deployment has one |
| `FERROTERM_NTS_USERNAME` | Your account name |
| `FERROTERM_NTS_PASSWORD` | Your account password |

The file form takes the same four field names in a JSON object, and the
configuration points at it with `{"from": "file", "path": "/run/secrets/nts.json"}`.

The add-on reads the token endpoint from
`https://terminologieserver.nl/fhir/.well-known/smart-configuration` and logs
in with the grant Nictiz documents: the password grant with `cli_client`. If
you configured a client secret, the client-credentials grant is tried first and
falls back to the password grant when the realm refuses it. The access token is
refreshed a minute before it expires, and a refused refresh, which is what a
spent 24 hour refresh token gives, logs in again from the stored credentials.
An unattended run on any day needs no person.

### The first run

1. Get the account, accept the terms, and confirm which systems your licences
   unlock.
2. Put the credentials where the add-on reads them, and keep them out of the
   configuration file.
3. Subscribe by canonical identifier. The defaults are the SNOMED CT
   Netherlands edition (`http://snomed.info/sct/11000146104`), LOINC
   (`http://loinc.org`), UCUM (`http://unitsofmeasure.org`), and ICD-10
   (`http://hl7.org/fhir/sid/icd-10`). Name the others once you see the
   canonicals the feed publishes for them.
4. Run once and read the run record before you activate anything. It lists
   every entry the feed offered, what was taken, and the reason for each entry
   left behind: a system you did not subscribe to, a release you already serve,
   or a system the service offers only as Ontoserver's binary index, which is
   an internal package format and is reported as not syndicable.
5. Check the corrections. One Nictiz map publishes `experimental` as the string
   `"true"`, and the add-on writes the boolean FHIR declares before the file is
   served. Every correction is listed with the element it changed; a file that
   needs none lands byte-identical.

### What is verified so far

The add-on is verified against fixtures only. Nobody has run it against the
live feed, because that needs an account, so the entry formats each system
arrives in, RF2 or a FHIR resource or the binary index, are not yet known.
Issue #581 stays open until a first real run confirms them.

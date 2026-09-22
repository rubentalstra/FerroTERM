# Syncing from a national terminology service

A national terminology service publishes its releases in an Atom syndication
feed: one entry per downloadable item, with the canonical identifier, the
version, the date, and a digest. The FerroTERM sync service reads such a feed
on a schedule, takes what you subscribed to, builds or stages it, and asks the
server to reload. It runs beside the server and never inside it, and it moves
content in one direction only.

This chapter is a stub. The service, its schedule, its activation modes, and
its run records are built under issue #579, and the full chapter arrives with
it. What follows is the part you can already read: the first source add-on.

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

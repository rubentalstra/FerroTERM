# Provenance: public terminology syndication listings

Fetched verbatim by `scripts/vendor/syndication-feeds.sh`, never hand-edited.
To refresh or extend the corpus, change the script, re-run it, and commit the
result. The fetcher is run by hand and never in CI: every operator republishes
its listing on its own cadence, so an automatic refresh would rewrite the
fixtures the corpus test pins, and a corpus test that fails after a refresh is
read before it is re-pinned.

Each file is a listing document: titles, dates, canonical URLs, checksums, and
byte sizes of downloadable packages. None of them carries terminology content.
No SNOMED CT concept, description, relationship, or reference set member
appears here, so the SNOMED CT content rule
(`.claude/rules/vendored-inputs.md`) holds by construction. The copyright of
each listing belongs to the operator named in the table, and the listings are
served without authentication for exactly this purpose: to be read by a
syndication client.

The MLDS listing stamps its feed-level `updated` at request time, so its
SHA-256 moves on every fetch even when no entry has changed. Compare the
entries, not the digest, when judging whether a refresh brought new content.

| File | Source | Operator | Fetched | Bytes | SHA-256 |
|---|---|---|---|---|---|
| `ncts.xml` | <https://api.healthterminologies.gov.au/syndication/v1/syndication.xml> | Australian Digital Health Agency, National Clinical Terminology Service | 2026-09-22 | 123151 | `1d05b5bf93867b12689215935418a64c0b860ff50176f9f61a9ba66b412d3b19` |
| `nhs-england.xml` | <https://ontology.nhs.uk/production1/synd/syndication.xml> | NHS England, Ontology Server | 2026-09-22 | 185402 | `e7bd2a7f6aaf345515d8d40cd819d0b54d3751650518a30069dbdbb597742a13` |
| `mlds.xml` | <https://mlds.ihtsdotools.org/api/feed> | SNOMED International, Member Licensing and Distribution Service | 2026-09-22 | 367490 | `b01319f746f896b32fb4972b65a7e4956ad1663cbe3df5d418d784dd54614670` |

## What is not vendored, and why

This section names services so the gap in the corpus is legible. It is a record
of what was fetched and what could not be, and the crate's own code
(`crates/syndication/src/`) names no operator, country, or code system.

- The New Zealand Health Terminology Service listing
  (<https://nzhts.digital.health.nz/synd/syndication.xml>) is open, and about
  4 MB of it is one FHIR ValueSet entry after another. The corpus buys no
  parser coverage for that weight, so it stays out of the tree.
- The Nictiz Nationale Terminologie Server
  (<https://terminologieserver.nl/synd/syndication.xml>) and the Belgian
  federal terminology server
  (<https://apps.health.belgium.be/ontoserver/synd/syndication.xml>) both answer
  `401` with a `WWW-Authenticate: Bearer` challenge on the listing itself, so
  neither is vendorable without an account.

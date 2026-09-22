# terminology-syndication

A client for the Atom syndication dialect terminology services publish.

National terminology services announce their downloadable content in an Atom
feed (RFC 4287) carrying the NCTS Atom Syndication Format extensions, which
Ontoserver documents. This crate reads that feed into a typed model, decides
what a run takes from it, fetches one content item with its digest verified,
and offers the `Source` trait as the seam each service add-on implements.

Nothing here is specific to a code system, a country, or an operator. The feed
dialect is the same wherever it is served, and a service's identity lives
entirely in its add-on: where its feed is, and how a listing and a download are
authorized.

## Where it sits

`terminology-syndication` is one crate of
[FerroTERM](https://github.com/rubentalstra/FerroTERM), a pure-Rust FHIR
terminology server for SNOMED CT, LOINC, and other clinical code systems. The
crates are published so other projects can reuse them; the API is pre-1.0 and
moves with the FerroTERM release train. The Rust module path is
`terminology_syndication`. Documentation:
<https://docs.rs/terminology-syndication>.

## Licence

Business Source License 1.1 (`LICENSE`): free to read, build, modify, and
redistribute, free for non-production use and for non-commercial production
use; commercial production use needs a licence from the Licensor; each version
becomes Apache License 2.0 four years after it is published. Clinical
terminology content (SNOMED CT, LOINC, RxNorm, ICD, the national code systems)
is licensed by its publisher and is never part of this crate.

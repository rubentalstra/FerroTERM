# Memory index

- [Architecture decisions](architecture-decisions.md), locked design: SNOMED as OWL-EL ontology with an offline-classification / online-serving split; graph MODEL served from an index-materialized store (CSR adjacency + roaring transitive-closure bitmaps in redb); machine-generated FHIR R4/R4B/R5/R6; pure-Rust, no graph DB, no Elasticsearch, no JVM
- [Owner work style](owner-work-style.md), research-first and evidence-based; from-first-principles, skeptical of the legacy way others build CT servers; confirm foundational decisions before scaffolding; the graph model is right, but implemented as a materialized index, not a graph database or live traversal
- [Official name FerroTERM](official-name-ferroterm.md), FerroTERM (ferroterm.eu) is the official name since 2026-09-02, Notio was the codename; FerroTERM in prose, `ferroterm` in identifiers
- [Local SNOMED release](local-snomed-release.md), the owner's licensed NL edition RF2 lives at data/snomed/ (gitignored), for local development and testing only; fixtures stay synthetic
- [Repo merge gates](repo-merge-gates.md), main needs one approving + code-owner review, signed commits, and the `conclusion` check; Claude stacks PRs and the owner merges; the Roadmap board did not exist as of 2026-09-02
- [Release cut cadence](release-cut-cadence.md), cut the release PR the moment a milestone hits zero open issues, then push the signed tag; the owner flagged v0.0.1 sitting uncut while v0.0.2 work merged
- [Container image decisions](container-image-decisions.md), distroless static base pinned by digest, numeric user 65532, docker/Dockerfile with a root .dockerignore, reusable L3 image lane with syft SBOMs per platform and actions/attest, Linux-only release targets, GHCR quirks
- [Post-release verification pause](post-release-verification-pause.md), after the v0.0.3 cut, stop and verify binaries, image, L3 provenance, SBOMs, signatures as a consumer before new work; the owner wants the supply chain proven
- [Performance bar](performance-bar.md), point reads and operations under 1 ms, NL ingest under 60 s, measured with criterion; never call a millisecond figure fine (#77)

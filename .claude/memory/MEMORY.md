# Memory index

- [Architecture decisions](architecture-decisions.md), locked design: SNOMED as OWL-EL ontology with an offline-classification / online-serving split; graph MODEL served from an index-materialized store (CSR adjacency + roaring transitive-closure bitmaps in redb); machine-generated FHIR R4/R4B/R5/R6; pure-Rust, no graph DB, no Elasticsearch, no JVM
- [Owner work style](owner-work-style.md), research-first and evidence-based; from-first-principles, skeptical of the legacy way others build CT servers; confirm foundational decisions before scaffolding; the graph model is right, but implemented as a materialized index, not a graph database or live traversal
- [Codename Notio](codename-notio.md), "Notio" is a temporary codename (Latin "concept"); the official name is not set; repo/dir/branding all say notio for now

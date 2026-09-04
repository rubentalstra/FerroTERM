# classification

The readers for statistical classifications: ClaML (ISO 13120; WHO ICD-10 and
its national translations, ICPC-2) and the NCHS ICD-10-CM tabular release.
Hand-written; the FHIR ICD page (<https://hl7.org/fhir/R4B/icd.html>) and the
ClaML DTD are the authority.

- Both readers produce one `Classification` model (chapters, blocks,
  categories, and subcategories, each with one parent, labelled and annotated
  by rubric kind); `ferroterm-build` turns that model into the served
  artifacts, so nothing downstream knows which file format a system came in.
  omits it gets it inserted after the third character.
- The releases are licence-gated or bring-your-own and never committed;
  fixtures are shaped like the files, with invented content.

# notio-rf2

The SNOMED CT RF2 loader and the typed component model. Hand-written; the
RF2 release file specification is the authority
(<https://confluence.ihtsdotools.org/display/DOCRELFMT>).

- Stream every RF2 file with `csv` (tab-delimited, no quoting); never load a
  whole file into memory.
- `ConceptId`, `DescriptionId`, `RelationshipId`, and `RefsetId` are distinct
  newtypes over the SCTID integer; never pass a bare integer where a typed id
  belongs (`.claude/rules/reliability.md`).
- Snapshot semantics: one row per component id, `active` and
  `effectiveTime` carried as typed fields, never dropped.
- Fixtures are shaped, synthetic RF2 only. Never commit content from a
  licensed release (`.claude/rules/vendored-inputs.md`).

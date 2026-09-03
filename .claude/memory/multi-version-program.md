---
name: multi-version-program
description: "v0.0.9 (R4/R5/R6 endpoints, XML) starts with a neutral-core refactor of the terminology operations; the design and the order of work, decided 2026-09-03 after the v0.0.8 cut"
metadata: 
  node_type: memory
  type: project
  originSessionId: 1fab17b9-946d-49fc-af2e-4719ce97bc31
  modified: 2026-09-03T11:58:45.875Z
---

State on 2026-09-03: v0.0.8 (ECL) is tagged; v0.0.9 holds #113 (R4), #114 (R5, unlocks the LOINC suite cases of #13), #115 (R6 ballot), #116 (XML), and the data-gated #12/#13/#15/#18/#19.

The engine's operations are written in `ferroterm_fhir::r4b` types (49 imports across `crates/ferroterm-terminology` and `app/ferroterm-server`). The decided design (the refactor issue filed at the cut, blocking #113/#114/#115): version-neutral operation inputs and outcomes in the engine (`LookupInput`/`LookupOutcome`, `ExpandInput`/`ExpansionOutcome`, `ValidateCodeInput`/`ValidationOutcome` with issues, `TranslateInput`/`TranslationOutcome`, `SubsumesInput`/`SubsumptionOutcome`), and one wire module per served version under `app/ferroterm-server/src/{r4,r4b,r5,r6}/` that decodes the version's generated request, maps to the input, and maps the outcome into the version's generated response, so an endpoint emits exactly what its `OperationDefinition` declares by construction. Metadata renders per version from the neutral registry. The generated R5 contracts differ from R4B additively (`useSupplement`, `definition`, `additionalUse`, `source` on lookup; `issues`, `code`, `system`, `version` on validate-code; `property` on expand).

**Why:** the owner wants every version served from one server (`docs/architecture.md`); the tx-ecosystem suite and the LOINC cases are written in R5 shapes, so R5 is the value driver.

**How to apply:** do the refactor first with `/r4b` behaviour pinned by the existing suites and the 120-case general pass list; then R4 (#113) as the first new wire module, then R5 (#114) and rerun the tx.fhir.org and general modes against `/r5`. See [[milestone-autonomy]], [[release-cut-cadence]].

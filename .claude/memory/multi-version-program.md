---
name: multi-version-program
description: "v0.0.9 (R4/R5/R6 endpoints, XML): the neutral core and R4/R5 are merged; #154 decided (b) on 2026-09-03, the ecosystem IG overlay program is #159 to #163; then #116 XML and #115 R6"
metadata: 
  node_type: memory
  type: project
  originSessionId: 1fab17b9-946d-49fc-af2e-4719ce97bc31
  modified: 2026-09-03T11:58:45.875Z
---

State on 2026-09-03 (late night): #154 is closed, all five slices merged (#166 overlay, #170 outputs, #171 lookup, #172 negotiation, #173 inferSystem/lenient); #174 (wildcard versions, PR #175) is in flight. `/r5` passes 201 of 670, `/r4b` and `/r4` 198 (from 123/49/49 in the morning). The remaining `/r5` clusters: validate-code parameter sets (supplement scoping, `codeableConcept` echo), issue wording versus the reference server, the `$expand` clusters, `$subsumes` "Unknown Operation" (runner-side, to adjudicate), `$translate`. Each crate change bumps the crate line (0.1.5 now).

The owner decided #154 as **(b)** on 2026-09-03: the terminology ecosystem IG 1.9.3 requirements (<https://hl7.org/fhir/uv/tx-ecosystem/1.9.3/requirements.html>) are adopted as a cited extension of every version's operation surface. The program is five sub-issues of #154: #159 the generator overlay (pre-adopt the R6 parameters from the vendored R6 package, declare the IG-only ones in the override map: `x-caused-by-unknown-system`, lookup `code`/`system`/`abstract`; unimplemented inputs refused `not-supported`, never ignored), then #160 version negotiation on ValueSet/$validate-code, #161 `inferSystem` and `lenient-display-validation`, #162 the validated code/system/version/issues and `x-caused-by-unknown-system` on R4/R4B, #163 lookup `code`/`system`/`abstract`. The `/r5` 400 cluster (136 cases) is the undeclared parameters; `sourceSystem` on $translate is R6 too. After #154: #116 (XML), then #115 (R6, blocked by #114). The server instantiates each version's wire from one macro set (`app/ferroterm-server/src/version/`), R5 with its own map (`app/ferroterm-server/src/r5/`).

The engine's operations are written in `fhir_types::r4b` types (49 imports across `crates/fhir-terminology` and `app/ferroterm-server`). The decided design (the refactor issue filed at the cut, blocking #113/#114/#115): version-neutral operation inputs and outcomes in the engine (`LookupInput`/`LookupOutcome`, `ExpandInput`/`ExpansionOutcome`, `ValidateCodeInput`/`ValidationOutcome` with issues, `TranslateInput`/`TranslationOutcome`, `SubsumesInput`/`SubsumptionOutcome`), and one wire module per served version under `app/ferroterm-server/src/{r4,r4b,r5,r6}/` that decodes the version's generated request, maps to the input, and maps the outcome into the version's generated response, so an endpoint emits exactly what its `OperationDefinition` declares by construction. Metadata renders per version from the neutral registry. The generated R5 contracts differ from R4B additively (`useSupplement`, `definition`, `additionalUse`, `source` on lookup; `issues`, `code`, `system`, `version` on validate-code; `property` on expand).

**Why:** the owner wants every version served from one server (`docs/architecture.md`); the tx-ecosystem suite and the LOINC cases are written in R5 shapes, so R5 is the value driver.

**How to apply:** do the refactor first with `/r4b` behaviour pinned by the existing suites and the 120-case general pass list; then R4 (#113) as the first new wire module, then R5 (#114) and rerun the tx.fhir.org and general modes against `/r5`. See [[milestone-autonomy]], [[release-cut-cadence]].

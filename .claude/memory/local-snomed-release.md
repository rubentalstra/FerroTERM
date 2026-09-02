---
name: local-snomed-release
description: "The owner's licensed SNOMED CT NL edition RF2 release lives at data/snomed/ in the ferroterm repo (gitignored), for local development and testing only"
metadata: 
  node_type: memory
  type: project
  originSessionId: 1fab17b9-946d-49fc-af2e-4719ce97bc31
  modified: 2026-09-02T14:53:21.700Z
---

The owner holds an MLDS licence (https://mlds.ihtsdotools.org) for the SNOMED CT Managed Service NL edition and, on 2026-09-02, moved the release archive `SnomedCT_ManagedServiceNL_PRODUCTION_NL1000146_20260630T120000Z.zip` out of the FerroEHR project into `data/snomed/` in the ferroterm repo, unpacked (Snapshot + Full, about 4.3 GB). `/data/` and `SnomedCT_*` are gitignored.

**Why:** The engine needs a real RF2 release to develop and test against (ferroterm-build, the Snowstorm/Hermes comparison), and the licence permits local development use; the repository must never ship SNOMED content.

**How to apply:** Point RF2 loading and build tests at `data/snomed/<release>/Snapshot/Terminology/` and `Refset/` locally. Never copy any concept, description, or term from it into a committed fixture; fixtures stay synthetic. Related: [[architecture-decisions]].

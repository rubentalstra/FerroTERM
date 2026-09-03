---
name: license-busl
description: "On 2026-09-03 the owner changed the project licence from MIT to the Business Source License 1.1 (no hosted-service offering, no for-fee resale; Apache 2.0 four years after each release); every SPDX header, manifest, badge, label, and page must say BUSL-1.1, guarded by scripts/checks/versions.sh"
metadata: 
  node_type: memory
  type: project
  originSessionId: 1fab17b9-946d-49fc-af2e-4719ce97bc31
  modified: 2026-09-03T12:09:50.519Z
---

The owner asked on 2026-09-03: "create a license change to something that people can not resell it ... also no SaaS ... everything needs to be updated right now". The choice made: Business Source License 1.1 (SPDX `BUSL-1.1`), Licensor Ruben Talstra, Additional Use Grant allowing production use (including inside products and services you operate) but not offering the software to third parties as a hosted or managed service nor selling it as a product in its own right, Change Date four years from each version's publication, Change License Apache 2.0. Releases up to v0.0.8 stay MIT as published.

Alternatives considered: Elastic License 2.0 (no expiry, blocks managed services but not resale of copies), Functional Source License 1.1 (two-year conversion, "competing use" test), PolyForm Shield (permanent, competition test), AGPL (does not stop SaaS), Commons Clause (ambiguous, avoided).

**Why:** the owner wants no resale and no SaaS by others; BUSL's Additional Use Grant states exactly that, and the Change License keeps the code eventually open.

**How to apply:** new files carry `SPDX-License-Identifier: BUSL-1.1`; the `versions` CI job fails on any stale MIT claim in the project's own files (third-party files such as `website/book/mermaid.min.js` and the vendored inputs keep their own licences). A licence question from the owner about terms is theirs to decide; present options with trade-offs. See [[release-cut-cadence]].

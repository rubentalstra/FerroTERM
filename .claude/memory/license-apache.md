---
name: license-apache
description: "On 2026-09-03 the owner moved the project licence from MIT to the Business Source License 1.1 (PR #147) and reverted it the same day for the Apache License 2.0; every SPDX header, manifest, badge, label, and page must say Apache-2.0, guarded by scripts/checks/versions.sh"
metadata: 
  node_type: memory
  type: project
  originSessionId: 1fab17b9-946d-49fc-af2e-4719ce97bc31
  modified: 2026-09-03T12:31:49.979Z
---

The licence of the project's own code is the Apache License 2.0 (`LICENSE` verbatim, `NOTICE` with the copyright and the SNOMED CT trademark note). It was MIT until 2026-09-03; a Business Source License 1.1 change (#147, "no resale, no SaaS") merged that day and was reverted within the hour when the owner decided "we should make it Apache 2.0 straight away". Releases up to v0.0.8 stay MIT as published.

**Why:** the owner weighed a source-available licence and chose an open one; Apache 2.0 gives the patent grant and the contribution terms (§5) MIT lacks.

**How to apply:** new files carry `SPDX-License-Identifier: Apache-2.0`; the `versions` CI job fails on any stale MIT claim in the project's own files (third-party files keep their own licences). A licence question from the owner is theirs to decide; present options with trade-offs, and expect the answer to be revisited. See [[release-cut-cadence]].

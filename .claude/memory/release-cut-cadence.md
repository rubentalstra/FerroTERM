---
name: release-cut-cadence
description: Cut the release the moment a milestone reaches zero open issues; the owner noticed v0.0.1 was never cut while v0.0.2 work had already merged
metadata: 
  node_type: memory
  type: feedback
  originSessionId: 1fab17b9-946d-49fc-af2e-4719ce97bc31
  modified: 2026-09-02T18:58:08.387Z
---

When a `vX.Y.Z` milestone reaches zero open issues, open the release PR right
away (workspace version bump in the root `Cargo.toml` and the path-dependency
versions, `Cargo.lock`, `CITATION.cff`, `docs/VERSIONS.md`, and the
`## [X.Y.Z] - date` changelog section), and after the owner merges it push a
GPG-signed tag (`git tag -s vX.Y.Z origin/main`) so `release.yml` publishes.
The tag ruleset requires signatures; `release.yml` refuses a tag whose version
differs from the first `^version = ` line of the root `Cargo.toml` or whose
changelog section is missing.

**Why:** On 2026-09-02 the owner wrote "you did not cut an release from the
0.0.1 and now starting 0.0.2 already": v0.0.1 sat at zero open issues while
four v0.0.2 issues merged, so the release had to absorb them (they moved to
v0.0.1 so milestone and release describe the same contents).

**How to apply:** Check the milestone count after every merge report; if it is
zero, the next unit of work is the release PR, before picking up the next
milestone's issue. See [[repo-merge-gates]].

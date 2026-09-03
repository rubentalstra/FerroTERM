---
name: release-cut-cadence
description: Cut the release the moment a milestone reaches zero open issues, and bump the version in EVERY place (README, landing page, book too); scripts/checks/versions.sh guards it since 2026-09-03
metadata:
  node_type: memory
  type: feedback
  originSessionId: 1fab17b9-946d-49fc-af2e-4719ce97bc31
  modified: 2026-09-03T09:32:09.439Z
---

When a `vX.Y.Z` milestone reaches zero open issues, open the release PR right
away, then push a GPG-signed tag (`git tag -s vX.Y.Z`) after it merges so
`release.yml` publishes. The release PR bumps the version everywhere the
release is named, not only the manifests: root `Cargo.toml` (workspace and
path-dependency versions), `Cargo.lock`, `CITATION.cff`, `compose.yaml`,
`docs/VERSIONS.md`, `README.md` (image tag, roadmap list), the landing page
`website/landing/index.html` (JSON-LD `version`, the status line, the status
panel), the book `website/book/src/**` (install, verifying-releases,
introduction, comparison, examples, hardware-sizing, what-ferroterm-is), and
the `## [X.Y.Z] - date` changelog section. Run `bash scripts/checks/versions.sh`
before pushing: its "stale product versions" sweep fails on any lower version
string in those files (CI `versions` job, required).

**Why:** On 2026-09-02 the owner wrote "you did not cut an release from the
0.0.1 and now starting 0.0.2 already". On 2026-09-03, after v0.0.7, the owner
wrote "you have not updated all the version number everywhere": the README,
landing page, and book still said 0.0.6 after the tag was pushed, fixed in
PR #139 together with the guard.

**How to apply:** Check the milestone count after every merge report; if it is
zero, the next unit of work is the release PR. In that PR, `git grep` the old
version across the whole tree (excluding CHANGELOG.md and Cargo.lock) and run
`scripts/checks/versions.sh` before opening it. See [[repo-merge-gates]] and
[[milestone-autonomy]].

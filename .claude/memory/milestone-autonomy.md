---
name: milestone-autonomy
description: "Owner directive (2026-09-03): work every milestone end to end autonomously, cut each release when its milestone empties, move stragglers forward, file new milestones for new issues"
metadata:
  type: feedback
---

Work the milestones in order (v0.0.4, then v0.0.5 LOINC and UCUM, v0.0.6
ICD-10 family and RxNorm, v0.0.7 ATC, ICPC, ICD-11, and the Dutch national
systems) without checking in between them: finish the open issues of a
milestone, open the `release: vX.Y.Z` PR the moment it empties (moving
issues that need the owner or a later layout to the next milestone with a
comment saying why), then start the next milestone. New work found on the way
gets an issue in the current milestone; when it does not fit any planned
release, create a new milestone for it rather than leaving it unscheduled.

**Why:** The owner wrote on 2026-09-03: "when you are fully finished with
this milestone i want you to do again this cut and release way and then go
to the next milestone please and just go through all of them ... keep going
create new milestone for new issues? please go autonomously".

**How to apply:** Only the signed tag needs the owner (they tag the merged
release commit themselves); everything else, including the release PR with
auto-merge, proceeds without asking. See [[release-cut-cadence]] and
[[repo-merge-gates]].

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

**How to apply:** Everything proceeds without asking, the signed tag
included: on 2026-09-03 the owner said "you can of course also cut the
release just run the same commands", and this clone signs with the
configured GPG key (`git tag -s vX.Y.Z -m vX.Y.Z <merge commit> && git push
origin vX.Y.Z`), which triggers `release.yml`. Close the milestone with the
cut (the owner noticed v0.0.4 left open). See [[release-cut-cadence]] and
[[repo-merge-gates]].

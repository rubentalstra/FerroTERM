---
name: openehr-archetype-canonical-thread
description: The owner's openEHR discourse thread (2026-09-12) asking what canonical URL an archetype-local value set gets; the community view backs FerroTERM's opaque-canonical stance from #547
metadata:
  type: reference
---

The owner opened <https://discourse.openehr.org/t/what-should-the-canonical-url-of-an-archetype-local-value-set-be/17292>
on 2026-09-12 after #547 (archetype terminology served through the ordinary
operations). No openEHR spec defines the URL; Archetype Identification's
"A Reliable URI for Knowledge Resources" section is still "To Be Continued".

Replies so far: Ian McNicoll mints `http://openehr.org/archetypes/{archetype_id}/CS`
in wt2xt (not wedded; wants ADL2 principles; notes template clone nodes
constrain the same value set differently). Seref: once domains diverge the
archetype concept is no longer assumed identical, so two minted canonicals
are correctly two identities. That matches the server's stance in
`docs/terminologies.md` (opaque canonical, served side by side, load refuses
on one canonical at one version from two directories).

Open point: if tooling converges on `http://openehr.org/...`, the doc's
"a producer mints under a domain it owns" line no longer describes practice.
Re-check the thread before touching that section. See [[architecture-decisions]].

# Memory index: leptos-reviewer

Ported from the sibling FerroEHR viewer programme, where each entry was
confirmed against pinned crate source or a live gate failure. **The FerroTERM
viewer is client-side rendered** (`docs/viewer.md` §1), so every entry carries a
`csr` classification in its front matter: `still-applies`, `changed`, or
`moot`. A moot entry is kept for the record, never deleted, because the fact it
records is true and the reasoning is reusable.

## Still applies, unchanged

- [Polled resource needs Transition](polled-resource-needs-transition.md): an
  interval-refetched or filter-refetched resource read under `<Suspense>`
  flashes its fallback; use `<Transition>`
- [Plain `<a>` IS client-side nav](internal-nav-uses-plain-anchor.md):
  leptos_router intercepts every same-origin anchor through a window click
  handler; `rel="external"` is the only opt-out
- [Same-route param nav keeps the view](router-same-route-param-nav.md):
  declaration-order first match; the same `<Route>` id means params update and
  the body never re-runs, so untracked params go stale
- [Router never intercepts form submits](leptos-router-form-interception.md): a
  plain `<form>` is safe; `<Form method="GET">` to the same path short-circuits
  `rebuild`
- [Builder signal plus struct_ver](builder-signal-struct-ver.md): the
  focus-preserving deep-tree editing pattern, for the taxonomy tree
- [Tree editor state bleeds on positional keys](directory-tree-editor.md):
  positional `<For>` keys leak collapse and rename state to the sibling that
  shifts into position, and a refetch re-seeds an editor mid-edit

## Changed under CSR (the hydration half is gone, the rest stands)

- [thaw hydration hazards](thaw-hydration-hazards.md): the mismatch is gone;
  the method survives, verify a widget's source before trusting its `id`/`for`
- [Confirmed-good patterns](w2-confirmed-good-patterns.md): the auth half is
  moot; `.into_any()` erasure, fixed-size integers, and the theme effect stand
- [Tabbed screen pattern](tabbed-screen-pattern.md): always-mounted bodies were
  a hydration device; tab-gated resource sources are now the whole point
- [Redirect paths must be encoded](redirect-path-must-be-percent-encoded.md):
  the `leptos_axum` panic is gone; percent-encoding every value that lands in a
  URL grows teeth, because system URIs and ECL land in FHIR request URLs
- [Journeys must click](no-js-journeys-must-click.md): there is no
  no-JavaScript journey; the review rule stands, a source-substring assertion
  proves nothing
- [Seed-once form idiom](seed-once-form-idiom.md): the hydration half is moot;
  a form seeded from a resource still must not overwrite what is being typed

## Moot under CSR (kept for the record)

- [thaw::Field random id](thaw-field-random-id.md): no server pass, so no
  mismatch; an explicit stable id is still wanted for labels and E2E selectors
- [thaw::Input name forwarding OK](thaw-input-name-forwarding-ok.md): recorded
  for `<ActionForm>` progressive enhancement, which does not exist here
- [chartistry Chart hydration](chartistry-chart-hydration.md): the client
  measurement gate was the hydration answer; the residue is an E2E wait
- [Redirect needs SsrMode::Async](redirect-needs-ssrmode-async.md): `SsrMode`
  does not exist under CSR

## Not ported

FerroEHR's `default-style-guard-untracked-blindspot` describes
`scripts/checks/default-style.sh`, a script with no counterpart in this
repository. Nothing in it transfers.

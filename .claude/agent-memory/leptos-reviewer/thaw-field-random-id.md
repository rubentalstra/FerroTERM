---
name: thaw-field-random-id
description: thaw::Field mints a Uuid::new_v4() id at setup; a hydration hazard in an SSR project, and kept here for the record because the practice it recommends is still right
csr: moot
metadata:
  type: reference
---

`thaw::Field` generates its label and input id with `Uuid::new_v4()` at
component setup (pinned thaw rev `0726a3d`, `thaw/src/field/field.rs:26`). It
renders that uuid onto the label as a **static** `attr:for` and provides it,
through `FieldInjection`, to the child `thaw::Input`, which renders it as a
**reactive** `id=Signal`.

**In an SSR project that is a defect:** the server emits uuid X and hydration
mints uuid Y, the static label `for` keeps X while the reactive input `id`
becomes Y, the label and input association breaks after hydration, and it can
trip a browser-console attribute-mismatch warning.

**Under CSR it cannot happen.** There is one render, in one process, so the id
is whatever was minted and the label points at it. `thaw::Field` is not banned
here on those grounds.

**Kept because the recommendation stands anyway.** Pass an explicit stable `id`
and `name` to every field. `Field::use_id_and_name` returns an explicit `id`
prop before falling back to the uuid, so `id="expand-filter"` is all it takes.
A stable id is what an E2E selector needs and what makes the label association
auditable, which `.claude/rules/leptos-ui.md` §9 requires for accessibility. A
random id per mount gives neither.

See [[thaw-hydration-hazards]] for the widget survey and
[[thaw-input-name-forwarding-ok]] for the `name` fact.

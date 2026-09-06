---
paths: ["app/ferroterm-viewer/**"]
---

# Leptos viewer rules (`app/ferroterm-viewer`, and any Leptos code)

The design is `docs/viewer.md`; this file is the enforceable discipline. The
oracle for every Leptos question is the official Leptos book
(<https://github.com/leptos-rs/book>, `main`, targets Leptos 0.8) through
`/leptos-lookup`, never memory and never the sibling project's SSR-era
patterns. Citations below are book chapters (`view/04_iteration`,
`async/12_transition`) and pinned crate documentation.

**The viewer is client-side rendered.** It compiles to WebAssembly with
`leptos/csr`, is built by Trunk, and runs entirely in the browser. There is no
server-side rendering, no hydration, no `#[server]` function, and no
`cargo-leptos`. Rules that exist in the sibling project for the `ssr`/`hydrate`
split do not apply here, and a rule reintroducing one is wrong.

## 0. The three mandates (absolute)

- **Rust only, zero hand-written JavaScript.** No authored `.js` files, no
  inline `<script>` bodies, no HTML `onxxx="..."` attributes carrying JS
  strings. Use an `on:` Rust listener. JS-wrapping crates (ECharts, Plotly
  bindings) are banned; charts are `leptos-chartistry`, which is pure Rust and
  SVG. The only JavaScript in the product is the `wasm-bindgen` bootstrap the
  toolchain generates.
- **The viewer is a FHIR client and nothing else.** It reaches FerroTERM only
  over the FHIR API, from the browser, same-origin. It depends on **no**
  workspace crate: not `fhir-terminology`, not `concept-graph`, not
  `concept-store`, not `sct-ecl`, not `ferroterm-server`, and not `fhir-types`.
  Anything the viewer can do, any client can do. That is the property the whole
  design exists to hold, and a dependency edge into the engine breaks it
  silently.
- **No code system is a special case.** Every page renders from
  `TerminologyCapabilities` and the operations. A page that names a system URI,
  assumes one hierarchy, or assumes one language reference set is a defect
  (`docs/viewer.md` §5).

The third mandate has a corollary worth stating: the viewer carries its own
small `serde` types for the fields it renders, and nothing beyond them. It
never mirrors a whole FHIR resource, and it never re-implements a terminology
operation in the browser.

## 1. Crate and build discipline

- One compilation target: `wasm32-unknown-unknown`, feature `csr`. The crate
  is a binary (`fn main` calling `leptos::mount::mount_to_body`), not a
  `cdylib`/`rlib` pair.
- The build is `trunk build`, driven by `index.html` and `Trunk.toml`. Keep
  `locked = true` so the build cannot re-resolve `Cargo.lock`, `filehash =
  true` so asset names are content-hashed, and `public_url = "/ui/"` so the
  emitted hrefs match where the server mounts the bundle
  (<https://github.com/trunk-rs/trunk/blob/main/guide/src/configuration/index.md>).
- Tailwind is Trunk's `rel="tailwind-css"` asset over the official standalone
  CLI, pinned with `[tools] tailwindcss`. No Node, no npm, ever.
- **WebAssembly is 32-bit: never `usize`/`isize` in a serialized type.** Use
  fixed-size integers in anything that crosses the wire or is stored.
- **Bundle size is gated.** Ship `[profile.release]` with
  `opt-level = "z"`, `lto = true`, `codegen-units = 1`; avoid `regex` and
  generics-heavy code on client paths (monomorphization bloat: factor a
  concrete inner function). A new dependency is justified against the bundle
  size it adds (`docs/viewer.md` §12).
- **Views are built in `.into_any()`-erased sections.** A monolithic `view!`
  tree over deeply nested `thaw` components blows rustc's layout-recursion
  depth at codegen. Break every screen into section functions bound to erased
  locals.
- No `unsafe` (the workspace forbids it), no `unwrap`/`expect` outside tests,
  `thiserror` for the viewer's own error enum, every public item documented,
  suppressions as `#[expect(lint, reason = "…")]` scoped to the smallest item.
  `.claude/rules/reliability.md` applies unchanged.

## 2. Reactivity

- Component functions are **setup functions and run once**. Anything dynamic
  in a view is a signal or a closure reading signals
  (`reactivity/interlude_functions`).
- Access discipline (`reactivity/working_with_signals`): `.get()`/`.set()` for
  cheap `Copy`-ish values; `.read()`/`.write()` guards or `.with()`/`.update()`
  for collections. Never `sig.get().is_empty()`, which clones the whole value.
  Never hold a `.read()` guard across a write or a `.write()` guard across a
  read.
- Signal depending on signal is a derived closure (`move || a.get() * 2`) or a
  `Memo`. **Writing one signal from an `Effect` that reads another is
  forbidden**; the book calls it out (`working_with_signals` §4). Effects exist
  only to sync with the non-reactive outside world (`localStorage`, the
  document title, logging). Most "I need an effect" cases are an event listener
  or a `leptos-use` primitive; check there first.
- Prefer local component state. Escalate in this order: **URL (router), then a
  context signal, then a `Store`** (`15_global_state`). Context values use the
  newtype pattern so the type is unambiguous; `expect_context` only where
  provision is structurally guaranteed.

## 3. Components and props

- Props that change over time are signal types, not plain values
  (`view/03_components`). A reusable component takes `#[prop(into)] x:
  Signal<T>` so a caller can pass a `ReadSignal`, a `Memo`, or a
  `Signal::derive`.
- Doc-comment every component and every prop; the macro turns them into real
  documentation.
- Child to parent is a callback prop or an `on:` listener on the component tag.
  Pass a `WriteSignal` down only where genuinely needed.

## 4. Views: iteration and control flow

- Dynamic lists use `<For each key children>` with a **stable, unique,
  data-derived key, never an index** (`view/04_iteration`). A concept id, a
  system URI, or a `ValueSet.url` is a real key. Signals stored in dynamic rows
  are `ArcRwSignal`.
- Key-includes-value re-renders the whole row (`view/04b_iteration`); prefer
  nested signals. Never `each = ….enumerate()` plus a `Memo` capturing the
  plain index inside `<For>`.
- Cheap conditional text or class is `move || if …` or `class:x=cond`.
  Expensive branches go behind `<Show when fallback>`. Divergent branch types
  use `Either`/`EitherOf3` or `.into_any()`.
- **An error never renders as nothing.** Resolve a `Result` where it arrives
  and render content or an explicit inline error view. A failed read renders
  inline, in the section whose data failed.
- **The taxonomy tree keeps UI state off positional keys.** Collapse and
  selection keyed by a `<For>` index bleed to the sibling that shifts into that
  position when the list changes (a confirmed hazard, see the agent memory).
  Key them by concept id.

## 5. Forms

- Controlled inputs use `prop:value` plus `on:input:target`, or the
  `bind:value`/`bind:checked`/`bind:group` sugar (`view/05_forms`). The `value`
  *attribute* only sets the initial value. A `<textarea>` needs child text plus
  `prop:value`; a `<select>` is driven by `prop:value` on the select.
- Uncontrolled forms use `NodeRef<html::X>` with `on:submit` and
  `ev.prevent_default()`.
- Parse user input yourself. `type="number"` is not validation.
- **A form whose result is shareable puts its state in the URL** (§8). The
  expansion runner, the search box, and every filter are URL state.
- Give every input an explicit, stable `id` and `name`, and associate the
  label with `for`. Do not rely on a widget to mint one.

## 6. Async data

- Loading is `LocalResource::new(|| async_fn())`. The book is explicit that
  `LocalResource` is the client-side-rendering resource and that
  `Resource::new()` "is used with SSR" and serializes a value from server to
  client (`async/10_resources`). **There is no server here, so every resource
  is a `LocalResource`.** The sibling project's rule that `LocalResource` is a
  deoptimization is an SSR rule; it does not apply.
- Read resources under `<Suspense>` on first load, and under **`<Transition>`
  whenever the resource refetches** on a filter, parameter, or interval change
  (`async/12_transition`). A `<Suspense>` over a refetching resource flashes
  its fallback on every reload. Every listing, search, and paged table in this
  viewer refetches, so `<Transition>` is the default there.
- Never fetch inside an `Effect` and write a signal. `spawn_local` is for
  genuine fire-and-forget only.
- **Only the visible tab fetches.** A tabbed screen gates each tab's resource
  source on the active tab, so opening a screen does not fan out one request
  per tab.

## 7. The FHIR client

- **One module owns every request.** All HTTP goes through `crate::fhir`, a
  thin `gloo-net` client that builds the URL, sets `Accept:
  application/fhir+json`, sends the request, and maps the answer. Never a
  second client, and never a `fetch` from a component body.
- **The base URL is derived from the page, never configured into the bundle.**
  The bundle is served by the server it queries, so the FHIR base is the
  document's own origin. A build-time base URL would make one artifact
  deployment-specific.
- **Every value interpolated into a URL is percent-encoded.** Code system
  URIs, ECL expressions, concept ids, and value set canonicals all contain
  characters that are structural in a URL. This is load-bearing, not cosmetic.
- **A refusal is an `OperationOutcome`, and the viewer renders it.** The
  server answers every failure with an `OperationOutcome` carrying `severity`,
  `code`, and `details`. Show the server's own diagnostic verbatim rather than
  a paraphrase, and never swallow a non-2xx into an empty view.
- **Errors are a typed enum** (`thiserror`), with the HTTP status as a
  `http::StatusCode` and the `OperationOutcome` carried as data. A caller that
  branches on the outcome gets a variant, not a substring match.
- The viewer's own FHIR types are the minimum it renders. Do not re-model the
  specification, and do not depend on `fhir-types`.

## 8. Router

- One `<Router>` at the root, `<Routes fallback=…>` with a real 404
  (`router/16`). The router base matches the server's mount point.
- Params and queries are typed: `use_params::<T>()` / `use_query::<T>()` with
  `#[derive(Params)]`, whose fields are `Option<T>`. Handle the `Err` and
  `None` cases; they are user input (`router/18`).
- **Filter, search, and pagination state lives in the URL** (`router/20`,
  `15_global_state`): shareable, refresh-safe, and walkable with the browser's
  back button.
- **A path or query param that can change without leaving the route is read
  reactively**, never `get_untracked` at setup. A navigation matching the same
  `<Route>` updates the params signal without re-running the component body
  (confirmed in `leptos_router` 0.8; see the agent memory), so an untracked
  read goes stale. The FHIR version switcher and the code system switcher are
  exactly such navigations.
- Navigation is `<A>` or a plain same-origin `<a href>`, both of which the
  router intercepts through its window-level click handler. Never write
  `window.location`. An anchor that opens a raw FHIR request needs
  `rel="external"`, or the router intercepts it and 404s.
- `<Form method="GET">` pointed at the **same** path does a client-side
  navigation whose `rebuild` short-circuits: the route body does not re-run.
  Combine it with a reactive query read, or use a plain `<form>` with an
  `on:submit` that navigates.
- Every routed page sets a `<Title>` through `leptos_meta`, from the component
  body and never by editing the document head by hand.

## 9. Accessibility

The bar is **WCAG 2.2 Level AA** (<https://www.w3.org/TR/WCAG22/>), checked per
slice, not at the end.

- Every control is keyboard reachable and operable, with a visible focus
  indicator. Nothing is mouse-only.
- The taxonomy tree implements the ARIA Authoring Practices tree view pattern
  (<https://www.w3.org/WAI/ARIA/apg/patterns/treeview/>): one tab stop, arrow
  keys to walk and expand, correct `role`, `aria-expanded`, `aria-selected`.
- Tables emit valid HTML with an explicit `<tbody>` and real `<th scope=…>`
  headers. No block element inside a `<p>`.
- No meaning is carried by colour alone; a status is a word as well as a tint.
  Contrast meets AA in both themes.
- A search or expansion result count is announced in a live region.
- Motion respects `prefers-reduced-motion`.

## 10. Testing and gates

- **Business logic lives outside components**, in plain types with ordinary
  unit tests: URL building, `OperationOutcome` flattening, capability-statement
  reading, the paging arithmetic, the tree model. Components stay thin. This is
  also what keeps the logic testable without a browser.
- Component tests are `wasm-bindgen-test` with `mount_to`; updates are async,
  so `tick().await` before asserting.
- **E2E is a merge gate, and it is Rust only**: `thirtyfour` (WebDriver)
  driving headless Chromium against the built image, journeys as plain
  `#[tokio::test]`s that skip with a reason when the base URL is unset. Every
  journey fails on any browser console error. Explicit waits on elements and
  conditions, never `sleep`. A flaky journey is fixed, never `#[ignore]`d.
  Playwright is JavaScript and the no-JavaScript mandate covers the test suite.
- **An E2E assertion on page source proves nothing.** A journey drives the real
  widget: type into it, click it, wait for the URL or the DOM to change.
- Gates for every viewer change (`/ui-gates`): `cargo fmt` and `leptosfmt`
  clean; `cargo clippy -p ferroterm-viewer --target wasm32-unknown-unknown
  --all-features -- -D warnings`; `cargo nextest run -p ferroterm-viewer`;
  `trunk build --release --locked` completing; the recorded bundle size not
  regressing.
- `console_error_panic_hook` is installed in `main`, so a panic names its
  source in the browser console.
- **Never weaken a gate to make a change pass.** A failing wasm clippy pass
  usually means a dependency cannot compile for the browser, which is the gate
  working.

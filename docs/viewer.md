# The FerroTERM Viewer

No FHIR or SNOMED CT specification governs a terminology server's user
interface: this is our own design. What the viewer *reads* is spec-bound, and
every citation below points at the FHIR specification, the Leptos book, or a
pinned crate's own documentation.

FerroTERM answers the FHIR terminology API and nothing else. Someone who has
just started it cannot see what it loaded, browse a hierarchy, or try an
expansion without writing a request by hand. Both of SNOMED International's
servers ship a dashboard for that, and a reader comparing the three notices the
gap. The viewer closes it.

`docs/architecture.md` is the design authority for the server. This document is
the design authority for the viewer, and it takes the architecture document's
constraints as given.

## 1. The one decision everything else follows from

**The browser is the FHIR client.** The viewer compiles to WebAssembly, runs
entirely in the browser, and issues every terminology request to the same
origin it was served from. The `ferroterm` binary serves the bundle as static
assets. There is one binary, one image, and no server-side rendering.

This is the shape Snowstorm Lite already uses, verified first-hand in
`IHTSDO/snowstorm-lite` at `master`, read 2026-09-06. Its
`src/main/resources/fhir/js/dashboard/routing.js` derives the FHIR base from
the page's own location, and `capability.js` then does
`fetch(this.fhirBaseUrl + '/metadata')` from the browser. The dashboard is
served by the application it talks to, and it talks to it over the public API
like any other client.

### Why this shape

- **The owner's constraint.** A UI-only OCI image beside the server image
  reads as a second product. The owner rejected it: a separate image "will
  also confuse people that SNOWSTORM used and it's basically part of the same
  project".
- **It makes the API-completeness claim structural.** Issue #366 asks that
  anything the UI can do, a client can do. Server-side rendering keeps that as
  a rule someone has to obey, because a server function can reach anything the
  process can reach. With the browser as the client there is no server
  function to abuse: the claim holds by construction, and the viewer becomes a
  standing demonstration that the public API is complete.
- **The release image has no shell.** `docker/Dockerfile` builds on
  `gcr.io/distroless/static-debian13:nonroot`. Two processes in one image
  would need a supervisor, and there is nothing in the image to be one. One
  binary avoids the question.
- **It removes the sibling project's worst recorded build hazard.** FerroEHR's
  viewer carries an `ssr`/`hydrate` feature split whose whole purpose is
  keeping server-only dependencies out of the WebAssembly bundle, and its
  crate manifest documents several ways that leaks. A CSR crate has one target
  and no gate to get wrong.
- **It removes a public HTTP surface.** FerroEHR's viewer rule opens by
  warning that every `#[server]` function is a publicly reachable endpoint that
  must enforce its own authentication. FerroTERM's viewer declares none, so
  there is nothing new to authenticate and nothing new to attack.

### The honest cost

**Server-side rendering is lost.** The Leptos book states the trade plainly:
CSR gives "faster build times, a quicker development cycle, and simpler
deployment. However, it can result in slower initial load times for users and
presents the same SEO challenges as typical JS SPAs. JavaScript must be
enabled in the browser for the CSR app to function"
(<https://github.com/leptos-rs/book/blob/main/src/getting_started/README.md>).
So: a slower first paint, no search-engine indexing, and no journey for a
reader with JavaScript disabled.

Each of those is acceptable here, and the reasons are specific rather than
general:

- **First paint.** The viewer is an operator and developer tool served from
  the same host as the API it browses, usually over a local network. It is not
  a public landing page.
- **Indexing.** The content a deployment loads is licensed. SNOMED CT is
  licensed by SNOMED International, so a deployment does not want its edition
  crawled and indexed.
- **No JavaScript.** The fallback for a reader without JavaScript is the FHIR
  API itself, which is the product. A no-JavaScript taxonomy tree is not
  reachable anyway.

FerroEHR keeps SSR because it carries OIDC sessions and clinical documents,
and because a clinician on a hospital network is a different reader. Neither
condition holds for a read-only terminology browser over a public API, and
Snowstorm Lite's dashboard has no server-side rendering either.

### What would change the decision

If the viewer ever needs to hold a credential, write to the server on a user's
behalf, or be indexed, the calculation changes and SSR comes back on the
table. None of those is planned. Record the change here if one arrives.

## 2. Packaging

### The bundle rides inside the binary

`trunk build --release` writes the bundle to `dist/`
(<https://github.com/leptos-rs/book/blob/main/src/deployment/csr.md>). That
directory is embedded into the `ferroterm` binary at compile time, so a
release tarball and the container image behave identically and there is no
"where did my UI go" failure mode from a moved directory. The image stays the
two-file image `docker/Dockerfile` already builds.

Embedding puts Trunk ahead of the server in the build order, and a fresh
clone has no `dist/` yet. `cargo build --workspace` must still succeed there,
so the embed is behind the server's `ui` feature, off by default and on in the
release lane, and the server answers `/ui` only when the bundle is compiled
in. No ordinary `cargo build`, `cargo clippy`, or `cargo nextest` run may
require a bundle that is not present.

The mechanism is the server's build script. With the `ui` feature on it walks
the bundle directory and writes a table of `Asset { path, bytes }` with one
`include_bytes!` per file, which `src/ui.rs` includes; the table is a static
in the binary and a request is a lookup in it, so no request path reaches the
filesystem. `FERROTERM_UI_BUNDLE` names the directory when a build stages it
elsewhere, and a directory it names that does not read is a `compile_error!`
telling you to run Trunk. The default directory may be absent, because
`cargo build --all-features` on a fresh clone must pass: the table is then
empty, the build warns, and the server mounts no `/ui` route. The workspace
denies `clippy::large_include_file`, so the build script writes a scoped
`#[expect(clippy::large_include_file, reason = "…")]` over the table exactly
when a file is large enough to fire the lint. Staging `dist/` beside the
binary and serving it with `tower-http`'s `ServeDir` stays the contingency if
the embed ever becomes the wrong trade.

### The routes the server gains

| Route | Answers |
|---|---|
| `GET /ui/` and `GET /ui/*` | the bundle, its assets, and the SPA fallback to `index.html` inside `/ui` only |
| `GET /` | a redirect to `/ui/` when the viewer is on; today's `OperationOutcome` `not-found` when it is off |

Everything outside `/ui` is untouched: `/health`, `/metrics`, `/r4`, `/r4b`,
`/r5`, `/r6`, and the catch-all `OperationOutcome` `not-found` that
`app/ferroterm-server/src/lib.rs` already installs. The SPA fallback is scoped
to `/ui` precisely so an unknown FHIR path keeps answering an
`OperationOutcome` rather than an HTML page.

`/ui` is chosen because the FHIR roots are already mounted at version prefixes
and the root path is free. Trunk is told the same prefix with
`public_url = "/ui/"`, which is what rewrites every asset href in the emitted
`index.html`
(<https://github.com/trunk-rs/trunk/blob/main/guide/src/configuration/index.md>).

### The switch

`FERROTERM_UI` joins the existing `FERROTERM_*` configuration in
`app/ferroterm-server/src/config.rs`. It defaults to on; `FERROTERM_UI=off`
drops the `/ui` routes and restores today's `/` behaviour, for a deployment
that wants an API-only surface. The switch drops routes rather than serving a
403, so a locked-down deployment presents no viewer at all.

### Two things the assets must not do

- **They must not be logged or measured as FHIR traffic.** The request-log and
  metrics middleware currently wraps every route. Asset requests are excluded,
  or the `/metrics` latency histograms stop describing the operations.
- **They must be cached honestly.** Trunk's `filehash = true` gives
  content-hashed asset names, so a long immutable `Cache-Control` on the hashed
  assets is true rather than a promise; `index.html` itself is served
  `no-cache`.

### Cross-origin is out of scope

Snowstorm Lite accepts a `?tx=` parameter pointing at another server. That
requires the target server to send CORS headers, which FerroTERM does not, and
it turns the viewer into a general FHIR client rather than this server's own
surface. Same-origin only.

## 3. The stack

Every version below was checked against crates.io on **2026-09-06**. Nothing
here is committed to the root `Cargo.toml` by this document; the slice that
creates the crate adds them, and re-checks each one at that moment.

| Crate or tool | Version | Why |
|---|---|---|
| `leptos` (feature `csr`) | 0.8.20 | the framework, client-side rendering only |
| `leptos_meta` | 0.8.6 | `<Title>` and document head from component bodies |
| `leptos_router` | 0.8.15 | client-side routing, URL as state |
| `thaw` (feature `csr`) | git rev `0726a3d6788f07e929996e77399d83655ffaacde` | the component library |
| `leptos-use` | 0.19.2 | isomorphic helpers (`use_interval_fn`, storage) |
| `leptos_icons` + `icondata_lu` + `icondata_core` | 0.7.1 + 0.1 + 0.1 | Lucide alone; the `icondata` umbrella pulls every pack |
| `leptos-chartistry` | 0.2.3 | pure-Rust SVG charts, no JavaScript |
| `gloo-net` | 0.7.0 | the browser fetch client |
| `wasm-bindgen` | 0.2.128 | the generated bootstrap |
| `console_error_panic_hook` | 0.1.7 | real stack traces in the browser console |
| `serde` / `serde_json` | pinned in the workspace | the FHIR JSON codec |
| `thirtyfour` (dev) | 0.37.5 | Rust-native WebDriver for the E2E journeys |
| Trunk (tool) | 0.21.14 | the CSR build tool |
| Tailwind CSS (tool) | pinned via Trunk `[tools] tailwindcss` | styling, no Node |
| `leptosfmt` (tool) | 0.1.33 | `view!` macro formatting |

Notes on three of these, each verified rather than assumed:

- **`thaw` stays on the pinned git rev.** crates.io has no 0.5 stable: the
  newest release is 0.4.8 (2025-08-03) and the only Leptos-0.8 release is
  `0.5.0-beta` (2025-05-03), which predates the `recursion_limit` fix. The
  pinned rev `0726a3d` (2026-05-07) is still the tip of `main`, confirmed with
  `git ls-remote https://github.com/thaw-ui/thaw` on 2026-09-06. It declares a
  first-class `csr` feature. Re-pin to crates.io when 0.5 stable ships.
- **Trunk 0.21.14 is the stable line.** 0.22.0-beta.2 (2026-07-24) is a
  prerelease and is not pinned.
- **`gloo-net`, not `reqwest`.** `reqwest` does compile for
  `wasm32-unknown-unknown` and switches to a fetch backend, but its docs record
  that "tls, cookie, blocking, as well as various `ClientBuilder` methods such
  as `timeout()` and `connector_layer()`" are disabled there
  (<https://docs.rs/reqwest>). The workspace takes `reqwest` with `rustls` for
  server-side tests; pulling that dependency tree into a WebAssembly bundle to
  reach an API that is one `fetch` away is not a trade worth making.
  `gloo-net` is the browser client and nothing else.

### The build tool is Trunk, not `cargo-leptos`

`cargo-leptos` states it outright in its own README: "Build server and client
for hydration (client-side rendering mode not supported)"
(<https://github.com/leptos-rs/cargo-leptos>). The Leptos book routes the two
cases in the same sentence: "Client-Side Rendering (CSR) with Trunk, suitable
for creating fast websites or **integrating with existing servers**, and
Full-Stack Server-Side Rendering (SSR) with `cargo-leptos`, ideal for
Rust-powered full-stack applications"
(<https://github.com/leptos-rs/book/blob/main/src/getting_started/README.md>).
An existing axum server that will serve the bundle is the first case, word for
word.

Trunk brings three things the SSR toolchain does not, each of which closes a
hazard FerroEHR paid for:

- `locked = true` in `Trunk.toml` requires `Cargo.lock` to be current, so the
  build cannot silently re-resolve the workspace. FerroEHR needs a wrapper
  script around `cargo leptos` for exactly this.
- `rel="tailwind-css"` compiles Tailwind with the official standalone CLI,
  which Trunk downloads itself at the version `[tools] tailwindcss` names. No
  Node, no npm.
- `filehash = true` and sub-resource integrity are on by default, so the
  emitted `index.html` carries hashed names and `integrity=` digests.

## 4. Reactivity under CSR

Two Leptos idioms differ from the sibling project's SSR-era patterns, and both
come straight from the book.

- **`LocalResource`, not `Resource`.** `Resource::new()` "is used with SSR"
  and serializes its value from server to client;
  `LocalResource` is for "client-side rendering or async tasks that must run
  in the browser"
  (<https://github.com/leptos-rs/book/blob/main/src/async/10_resources.md>).
  Every fetch in this viewer is a `LocalResource`. FerroEHR's rule that
  `LocalResource` is a deoptimization is an SSR rule and does not apply here.
- **`<Transition>` for reloads.** A resource that refetches on a filter,
  parameter, or interval change is read under `<Transition>` so the old data
  stays visible, rather than `<Suspense>`, which reverts to its fallback on
  every refetch
  (<https://github.com/leptos-rs/book/blob/main/src/async/12_transition.md>).
  This one is unchanged from the sibling project and matters more here,
  because every screen is a reload.

## 5. Code-system neutrality

FerroTERM serves the code systems listed in `docs/terminologies.md`. A browser
that assumes one hierarchy and one language reference set does not fit, so
neutrality is a structural property of the viewer rather than a rule someone
remembers:

- **Every screen renders from `TerminologyCapabilities`.** The viewer reads
  `GET /{version}/metadata?mode=terminology` and draws what each system
  declares: its versions and default, its content mode, whether it supports
  subsumption, its designation languages, its filters with their operators,
  and its `$lookup` properties. `crates/fhir-terminology/src/capabilities.rs`
  is what fills that document, so the viewer and the engine cannot drift.
- **An affordance appears only when the capability statement declares it.** A
  hierarchy pane renders when the system's declared filters include the
  hierarchy operators; otherwise the system's concepts render as a flat,
  searchable list. A language picker offers the languages the version
  declares, and nothing else. A `$translate` panel appears for a system that
  declares a concept map capability.
- **No code system name is hard-coded in a page.** SNOMED CT is not a special
  case in the viewer, in the same way it is not a special case in the engine
  (`docs/architecture.md` §5).

## 6. What only this server can show

Three things distinguish this viewer from Snowstorm Lite's dashboard. Each one
is a screen requirement.

1. **Four FHIR versions from one process.** `/r4`, `/r4b`, `/r5`, and `/r6`
   are separate roots with separate `CapabilityStatement`s and separate
   operation sets, generated per version from the vendored packages. The
   viewer reads all four and shows the differences it finds, for example that
   R4 and R4B declare no instance-level `$lookup` while R5 and the R6 ballot
   do, and that the R6 ballot removed `ConceptMap/$closure`.
2. **The artifact each system came from.** A deployment brings its own
   licensed release and the offline build turns it into an index directory.
   Which artifact, at which version, produced a served system is the first
   question an operator asks, and today it is not on the wire. Closing that is
   a server slice, listed in the checklist below.
3. **The conformance and benchmark figures the repository commits.** The
   tx-ecosystem pass lists under `conformance/tx-ecosystem/` and the latency
   claims in `bench/bars.json` with the runs under `bench/records/` are facts
   about the build, not about the running deployment. They ship as static JSON
   emitted from those committed files at build time, stamped with the release
   version, and the screen says plainly that they describe this build.

## 7. Accessibility

The bar is **WCAG 2.2 Level AA** (<https://www.w3.org/TR/WCAG22/>) on every
shipped screen. Concretely, and checked per slice:

- Every control is reachable and operable from the keyboard, with a visible
  focus indicator.
- The taxonomy tree follows the ARIA Authoring Practices tree view pattern
  (<https://www.w3.org/WAI/ARIA/apg/patterns/treeview/>): one tab stop, arrow
  keys to walk and expand, correct `role`, `aria-expanded`, and
  `aria-selected`.
- Tables carry real `<th>` headers with a scope, and an explicit `<tbody>`.
- No meaning is carried by colour alone. Status is a word as well as a tint.
- Text and interface contrast meets AA in both the light and dark themes.
- Motion respects `prefers-reduced-motion`.
- Every routed page sets a `<Title>` through `leptos_meta`, and a live region
  announces the result count after a search or an expansion.

The `accessibility` tracker label marks a slice with a specific obligation,
and a dedicated audit closes the programme.

## 8. Screen inventory

The reference scope is Snowstorm Lite's dashboard, read from
`IHTSDO/snowstorm-lite` at `master` on 2026-09-06. Its sections are
`resources/{codesystem,valueset,conceptmap}`, `syndication`,
`snomed-mini-browser`, `settings`, and `upload-sct`
(`js/dashboard/routing.js`), across thirteen dashboard modules and a 102,181
byte `index.html`. The full Snowstorm ships eleven modules and a 71,318 byte
`index.html`, and has neither `snomedBrowser.js` nor `settings.js`: its rich
browsing lives in the separate SNOMED CT Browser front end. Issue #366's
correction is confirmed. Lite is the richer of the two and is the one to
match.

Two of Lite's sections have no counterpart here. **Syndication** is Lite's
feed of installable editions; FerroTERM takes its content from an offline
build over a licensed release, so there is no feed to browse. **Upload SCT** is
the same story: `tools/ferroterm-build` does that, offline, once per edition.

| # | Screen | Route | Reads |
|---|---|---|---|
| 0 | Shell | all | `GET /health`; `GET /{v}/metadata` per version for the version switcher; theme and display language from `localStorage` |
| 1 | Overview | `/ui` | `GET /{v}/metadata?mode=terminology`: one card per system with its versions, default, content mode, subsumption, languages, and the artifact it came from |
| 2 | Code system detail | `/ui/systems/:url` | the same capability statement, plus `GET /{v}/CodeSystem?url=` for the published resource: declared filters with operators, `$lookup` properties, designation languages |
| 3 | Concept browser | `/ui/browse` | search through `ValueSet/$expand` with `filter`; concept detail through `CodeSystem/$lookup`; the hierarchy walk through `$expand` over the system's declared child filter. Renders a tree only for a system whose capability statement declares hierarchy operators |
| 4 | Expansion runner | `/ui/expand` | `ValueSet/$expand` by `url` or inline compose, with `filter`, `count`, `offset`, `displayLanguage`, `activeOnly`, `includeDesignations`; shows `expansion.total`, the echoed `expansion.parameter`, and pages through the result |
| 5 | Value sets | `/ui/valuesets` | `GET /{v}/ValueSet` search and read, with a link into the expansion runner |
| 6 | Concept maps and `$translate` | `/ui/conceptmaps` | `GET /{v}/ConceptMap` search and read; `ConceptMap/$translate` with source, target, and the returned `match` list with equivalences |
| 7 | Validate and subsume | `/ui/validate` | `CodeSystem/$validate-code`, `ValueSet/$validate-code`, `CodeSystem/$subsumes`; renders the `OperationOutcome` verbatim on refusal |
| 8 | FHIR versions | `/ui/versions` | the four `CapabilityStatement`s side by side: `fhirVersion`, the operations each resource declares, and the differences between them |
| 9 | Evidence | `/ui/evidence` | the static conformance and benchmark JSON emitted at build time |
| 10 | Settings | `/ui/settings` | the FHIR base in use, the display language, page size, theme. `localStorage` only, per viewer |

**The request disclosure is shell-level, not a screen.** Every data section can
reveal the exact FHIR request it issued, as a copyable URL and a `curl` line.
That is the cheapest possible demonstration of the boundary this design
exists to hold: the reader sees that the page did nothing they cannot do
themselves.

### Deliberately out of scope

- **Writing FHIR resources from the viewer.** The server exposes `CodeSystem`,
  `ValueSet`, and `ConceptMap` create, update, and delete, and the viewer does
  not call them. Those are unauthenticated in the server today, and a UI that
  invites a destructive call is a different product with a different security
  design.
- **Browsing another server (`?tx=`).** Same-origin only, §2.
- **Syndication and edition installation.** No feed exists; the offline build
  owns edition loading.
- **Server-side rendering, hydration, and a no-JavaScript journey.** §1.
- **Translating the viewer's own chrome.** The viewer renders the *content*
  languages a system declares; its own labels are English until someone asks
  for more.

## 9. What ports from FerroEHR, and what does not

FerroEHR's viewer is 52,796 lines of Rust over 23 screens for a clinical data
repository: sessions, OIDC, EHRs, compositions, templates, AQL, subscriptions,
audit. It shares no domain with a terminology browser. The split is therefore
sharp, and it favours porting the discipline rather than the code.

### Ports, adapted

| From FerroEHR | To FerroTERM | Adaptation |
|---|---|---|
| `.claude/rules/leptos-ui.md` | `.claude/rules/leptos-ui.md` | rewritten for CSR: the `ssr`/`hydrate` split, server functions, `<ActionForm>`, SSR modes, and progressive enhancement are gone; `LocalResource`, the fetch client, and the FHIR boundary replace them |
| `.claude/agent-memory/leptos-reviewer/` (17 hazards) | the same path | 16 ported and classified against CSR (§10); one dropped as tool-specific |
| `.claude/agents/leptos-reviewer.md` | the same path | the review priorities re-ordered around the FHIR boundary and bundle size |
| `.claude/agents/ui-implementer.md` | the same path | the gate list re-pointed at Trunk and the wasm target |
| `.claude/skills/leptos-lookup/SKILL.md` | the same path | unchanged in method; the cache path and the CSR chapters differ |
| `.claude/skills/ui-gates/SKILL.md` | the same path | the battery rewritten for one target and `trunk build` |
| `docker/viewer/Dockerfile` | `.github/workflows/release-build.yml` | **the reasoning ports; the commands and the location do not.** `docker/Dockerfile` compiles nothing, it copies binaries `release-build.yml` already built and attested, so the Trunk build belongs in that workflow, ahead of the `cargo auditable build` that embeds the bundle. What transfers is the record of why `cargo-chef` was removed (workspace members cannot survive the COPY boundary, so every source change recompiled the whole graph anyway) and how BuildKit cache mounts with `sharing=locked` bound peak memory. Its `cargo-leptos` invocation does not |
| the CI lane shapes in `build-image.yml`, `release-build.yml`, `ui-e2e-published.yml` | `ci.yml`, `release-build.yml`, `release-image.yml` | the shapes transfer: a wasm clippy pass, a formatter pass, a bundle build, an E2E job against a published artifact. The commands are Trunk's |
| `style/tailwind.css` token layer, `theme.rs` | the same shape | the token layer and the custom thaw brand ramp are a starting point, recoloured for FerroTERM |
| `components/`: `notice`, `page_header`, `data_table`, `empty_state`, `stat_card`, `tab_bar`, `surface`, `facts`, `format_view` | the same shape | domain-free presentation kits. They are re-derived rather than copied wholesale, because each carries CDR-specific copy and a `ViewerError` that does not exist here |

### Does not port at all

`server.rs`, `session.rs`, `session_client.rs`, `oidc.rs`, `auth.rs`,
`scopes.rs`, `cdr.rs`, `management.rs`, `admin.rs`, `tenants.rs`, and
`system_api.rs` have no counterpart. Under CSR there is no server binary, no
server function, no session cookie, and no back-end-for-front-end, so the
entire authentication and transport half of that crate is not a thing this
viewer has. `queries_api.rs`, `builder/`, `aql_text.rs`, `adl2.rs`,
`clinical.rs`, `subscriptions.rs`, and every `pages/` module are openEHR
domain code.

**Every screen in §8 is written from scratch.** The estimate that a large
amount ports is right about the scaffolding and the discipline, and wrong
about the screens.

## 10. The recorded hazards, re-audited under CSR

FerroEHR's `leptos-reviewer` agent memory holds seventeen confirmed hazards.
They are the most valuable thing in the port, because each was paid for once
already. Sixteen are carried over to
`.claude/agent-memory/leptos-reviewer/`; each file states its classification
in its own front matter, and the moot ones are kept with a note rather than
deleted.

| Hazard | Under CSR | Why |
|---|---|---|
| `polled-resource-needs-transition` | still applies | a refetched resource under `<Suspense>` flashes its fallback. A reactivity fact, unrelated to SSR, and the health pill polls |
| `internal-nav-uses-plain-anchor` | still applies | `leptos_router` installs a window-level click handler and intercepts every same-origin anchor. The `rel="external"` corollary now applies to links that open a raw FHIR request |
| `router-same-route-param-nav` | still applies | a navigation matching the same `<Route>` updates params without re-running the body, so an untracked param read goes stale. The version switcher and the system switcher are exactly such navigations |
| `builder-signal-struct-ver` | still applies | the focus-preserving deep-tree editing pattern is what the taxonomy tree needs |
| `leptos-router-form-interception` | still applies | the router never intercepts a native submit, and `<Form method="GET">` to the same path short-circuits `rebuild`. The expansion runner is a GET form to its own path, so this is load-bearing |
| `directory-tree-editor` | still applies | positional `<For>` keys bleed collapse state to the sibling that shifts into position, and a refetch re-seeds an editor mid-edit. The taxonomy tree is the same shape |
| `thaw-hydration-hazards` | changed | the hydration mismatch is gone; the method survives. Read a thaw widget's source before trusting what it renders for `id` and `for` |
| `w2-confirmed-good-patterns` | changed | the auth-guard half is moot. The `.into_any()` section erasure still applies, because the rustc layout-recursion limit is a codegen fact; so do the fixed-size-integer and theme-effect findings |
| `tabbed-screen-pattern` | changed | always-mounted bodies were a hydration-stability device and are no longer required. Gating each tab's resource on the active tab still applies and is now the whole point |
| `redirect-path-must-be-percent-encoded` | changed | the `leptos_axum::redirect` panic is gone with the server. The rule survives and grows teeth: system URIs, ECL, and concept ids all land in FHIR request URLs, and every one is percent-encoded |
| `no-js-journeys-must-click` | changed | there is no no-JavaScript journey. The residue is the review rule: an assertion on page source proves nothing, so a journey drives the real widget |
| `thaw-field-random-id` | moot | `thaw::Field` mints a `Uuid::new_v4()` id at setup. With no server pass there is no mismatch. Kept: an explicit stable id is still wanted for label association and for E2E selectors |
| `thaw-input-name-forwarding-ok` | moot | it recorded that `thaw::Input` forwards `name` so an `<ActionForm>` submits without WebAssembly. There is no `<ActionForm>` and no no-JavaScript path |
| `chartistry-chart-hydration` | moot | the chart self-gates on a client measurement, which was the hydration answer. The residue is an E2E fact: the chart renders a placeholder until its container is measured, so a journey waits on the drawn chart |
| `redirect-needs-ssrmode-async` | moot | `SsrMode` does not exist under CSR |
| `seed-once-form-idiom` | changed | the hydration half is moot. The refetch-versus-edit-in-progress half stands: a form seeded from a resource must not overwrite what the reader is typing |
| `default-style-guard-untracked-blindspot` | not ported | it describes `scripts/checks/default-style.sh`, a FerroEHR script with no counterpart here |

## 11. The build checklist

Ordered so the ported foundation lands first and every step after it is
independently shippable. Each row is one tracker sub-issue of #366, in
milestone v0.1.1.

| # | Slice | Port or new |
|---|---|---|
| 0 | The `.claude` discipline: the CSR rule file, the two agents, the two skills, the sixteen classified hazards, and this document | ported and adapted |
| 1 | The crate `app/ferroterm-viewer`: the Trunk build, Tailwind, the theme, the router and shell, the FHIR client module, the settings screen, and the gate script | scaffolding ported, wiring new |
| 2 | The server serves the bundle at `/ui` behind `FERROTERM_UI`, with the SPA fallback scoped, the assets excluded from the request log and metrics, and `/` redirecting | new |
| 3 | CI and CD: the wasm clippy pass, `leptosfmt`, the Trunk build, the recorded bundle size, and the bundle embedded by the release lane | lane shapes ported |
| 4 | The E2E harness and the first journey | harness ported, journeys new |
| 5 | The server declares the artifact each served system was loaded from | new, server-side |
| 6 | The overview screen | new |
| 7 | The code system detail screen | new |
| 8 | The concept browser: search, detail, hierarchy, code-system-neutral | new |
| 9 | The expansion runner, paged | new |
| 10 | Value sets, concept maps, and `$translate` | new |
| 11 | The `$validate-code` and `$subsumes` runners | new |
| 12 | The four FHIR versions side by side | new |
| 13 | The conformance and benchmark evidence screen | new |
| 14 | The accessibility conformance pass | new |
| 15 | The viewer in the book, with screenshots | new |

Slice 5 blocks slice 6, because the overview screen cannot show an artifact
the wire does not carry. Slice 1 blocks every screen. Slice 2 blocks the E2E
harness, which needs a served bundle to drive.

## 12. What CI and CD gain

The additions, to be recorded in `docs/ci-cd.md` when slice 3 lands:

- **`ci.yml`, a `viewer` job.** `cargo fmt` and `leptosfmt --check` over the
  crate, `cargo clippy --target wasm32-unknown-unknown --all-features -D
  warnings`, `cargo nextest run -p ferroterm-viewer` for the component-free
  logic, and `trunk build --release --locked`. The wasm target is the gate
  that matters: it is the only place a dependency that cannot compile for the
  browser shows up.
- **A recorded bundle size.** The compressed `.wasm` size is written to a
  committed file and compared on every build, the same shape
  `scripts/checks/bench-bars.sh` already uses for latency: a claim that never
  moves to match a slower build.
- **`ui-e2e`, a merge gate.** `thirtyfour` driving headless Chromium against a
  container built from the same Dockerfile, with the journeys as plain
  `#[tokio::test]`s. Every journey fails on a browser console error. Rust
  only: Playwright is JavaScript and the no-JavaScript-authored mandate covers
  the test suite.
- **The release lane.** The bundle is architecture-independent, so it is built
  once and embedded into each per-architecture binary before
  `release-build.yml` attests it. No new image, no new attestation subject,
  and no change to the SLSA Build L3 shape: the bundle becomes part of the
  binary the existing lane already signs.
- **`docker/Dockerfile` is unchanged.** It compiles nothing today: it copies
  the two binaries `release-build.yml` built and attested. The bundle rides
  inside the server binary, so the image gains no stage, no file, and no new
  `hadolint` surface.

## 13. Sources

- Leptos book, getting started and CSR wrap-up:
  <https://github.com/leptos-rs/book/blob/main/src/getting_started/README.md>,
  <https://github.com/leptos-rs/book/blob/main/src/csr_wrapping_up.md>
- Leptos book, CSR deployment:
  <https://github.com/leptos-rs/book/blob/main/src/deployment/csr.md>
- Leptos book, resources and transitions:
  <https://github.com/leptos-rs/book/blob/main/src/async/10_resources.md>,
  <https://github.com/leptos-rs/book/blob/main/src/async/12_transition.md>
- `cargo-leptos`, on hydration-only support:
  <https://github.com/leptos-rs/cargo-leptos>
- Trunk configuration and assets:
  <https://github.com/trunk-rs/trunk/blob/main/guide/src/configuration/index.md>,
  <https://github.com/trunk-rs/trunk/blob/main/guide/src/assets/index.md>
- `reqwest` WebAssembly support and its limits: <https://docs.rs/reqwest>
- FHIR `TerminologyCapabilities`:
  <https://hl7.org/fhir/R4B/terminologycapabilities.html>
- FHIR RESTful API and capabilities: <https://hl7.org/fhir/R4B/http.html>
- WCAG 2.2: <https://www.w3.org/TR/WCAG22/>
- ARIA Authoring Practices, tree view:
  <https://www.w3.org/WAI/ARIA/apg/patterns/treeview/>
- Snowstorm Lite dashboard, read 2026-09-06:
  <https://github.com/IHTSDO/snowstorm-lite/tree/master/src/main/resources/fhir>
- Snowstorm dashboard, read 2026-09-06:
  <https://github.com/IHTSDO/snowstorm/tree/master/src/main/resources/fhir>

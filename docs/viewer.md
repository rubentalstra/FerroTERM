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
| `leptos-use` | 0.19.2 | isomorphic helpers (`use_interval_fn`, storage) |
| `gloo-net` | 0.7.0 | the browser fetch client |
| `wasm-bindgen` | 0.2.128 | the generated bootstrap |
| `console_error_panic_hook` | 0.1.7 | real stack traces in the browser console |
| `serde` / `serde_json` | pinned in the workspace | the FHIR JSON codec |
| `thirtyfour` (dev) | 0.37.5 | Rust-native WebDriver for the E2E journeys, in `e2e/` |
| Trunk (tool) | 0.21.14 | the CSR build tool |
| Tailwind CSS (tool) | pinned via Trunk `[tools] tailwindcss` | styling, no Node |
| `leptosfmt` (tool) | 0.1.33 | `view!` macro formatting |

Notes on four of these, each verified rather than assumed:

- **There is no icon crate.** The screens draw twenty-one stroked outlines
  written in `components/icon.rs` (#476). The pinned alternative was
  `leptos_icons` 0.7.1 over `icondata_lu` 0.1.0, and both were built against
  the same call sites on one host: the crate came out 1,737 gzipped bytes
  heavier, 7,509 against 5,772. Tree shaking holds either way, and the
  sceptical reading was wrong: `icondata_lu` carries 1,599 icons and grepping
  the built `.wasm` for the path data of one the viewer does not reference
  finds nothing. The bytes were the smaller half of the decision.
  `leptos_icons::Icon` writes `role="graphics-symbol"` on every icon it draws
  and takes no class prop, so a decorative glyph needs an `aria-hidden` spread
  over a role the component insists on, and sizing goes through `width` and
  `height` props instead of the Tailwind scale the rest of the viewer uses.
- **There is no charting library.** `leptos-chartistry` 0.2.3 was pinned here
  as the pure-Rust SVG answer to the no-JavaScript mandate, and nothing had
  weighed it. The evidence screen (#414) weighed it: the same tree, the same
  `wasm-release` profile, one three-row two-series bar chart added and nothing
  else, on one host. The `.wasm` went from 451,872 to 584,100 gzipped, so the
  crate costs **132,228 gzipped bytes**, and the bootstrap grew 1,157 with it.
  That is a quarter of the whole ceiling for one dependency, and more than the
  finished viewer's other nine screens together. It also fits the screen
  badly: its axes are numeric, and every figure the evidence screen draws is
  named rather than numbered. So the screen draws no chart widget. Its figures
  are tables, and a proportional bar sits in the cell beside each number, drawn
  as a `<span>` whose width is a percentage and hidden from assistive
  technology, because the number it draws is already text in the same cell.
- **There is no component library.** The viewer used `thaw` for three widgets,
  a button, a spinner, and the config provider that themed them, and paid
  36,894 gzipped bytes for the whole library (§13, the bundle bar). Those three
  are now `components/button.rs`, `components/spinner.rs`, and a Tailwind
  `dark:` variant driven by a class on the document element. A component
  library is reconsidered only against a measured bundle cost and a use that is
  more than a handful of widgets.
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
   about the build, not about the running deployment. The crate's build script
   reads those committed files and writes the constant the bundle carries,
   stamped with the release version, and the screen says plainly that they
   describe this build. The script checks the pass lists against the mode table
   that records what each run covered, and stops the build with a
   `compile_error!` when they disagree or a file does not read, so no figure
   ships without a source.

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

The `accessibility` tracker label marks a slice with a specific obligation.
The audit that closes the programme is `e2e/tests/it/accessibility.rs`, which
measures every item above on every screen in both themes: it tabs through each
screen and reports any control the keyboard never reached and any stop the
browser drew no outline on, it computes the contrast ratio of every text the
browser painted, it reads the tables, labels, ids and headings out of the DOM,
and it checks that a search, an expansion and a validation each announce
themselves in a live region.

Two things there are read through a script, because WebDriver has no primitive
for either: which element focus landed on after a key press, and the colour the
browser painted. The second matters more than it sounds. Tailwind writes its
palette in oklch and a browser resolves `color` in the space the author wrote
(<https://www.w3.org/TR/css-color-4/#resolving-color-values>), so measuring the
token the source names would measure something no reader ever sees.

The pass found three failures and each was fixed rather than waived: the
version switcher's label at 4.35:1, its selected pill at 3.72:1, and the submit
button in the dark theme at 3.74:1. The submit button's class string was
written out five times, so the colour pairings more than one screen paints now
live in `app/ferroterm-viewer/src/styles.rs` and are measured once.

## 8. The design system

Every look decision used to be made in the screen that needed it:
`text-slate-600` appeared 72 times, `mt-3` 64 times, and a theme was a `dark:`
prefix on every element. The token layer replaces that. A screen names a role,
a type step and a spacing step, and nothing else.

**Colour is a role, not a colour.** `style/tailwind.css` declares each role as
a plain custom property on `:root`, redefines it once under `:root.dark`, and
maps it through `@theme inline`, which is how a Tailwind utility follows a
variable that changes under a selector
(<https://tailwindcss.com/docs/colors#referencing-other-variables>). The
selector is `:root.dark` rather than `:where(.dark)`: a zero-specificity
selector loses to `:root`, and a dark run drew the light surfaces until it was
fixed.

The roles are `fg`, `muted`, `faint`; `surface`, `raised`, `inset`; `line`,
`line-strong`; `accent` with its `accent-fg` and a soft pair; soft pairs for
`ok`, `warn` and `danger`; and the three the brand mark is drawn in. Nothing
outside that list names a colour.

**Type is six steps**, `display` through `micro`. Weight carries the hierarchy
below the top two steps and size carries it above them, so a screen does not
need a seventh size to make one heading louder than another.

**Spacing is four steps on an 8-point grid**: `tight`, `default`, `loose`,
`section`, and nothing between them. They reach Tailwind through the
`--spacing-*` namespace, so `mt-loose`, `gap-tight` and `px-section` are
ordinary utilities.

**Density is a density.** The four spacing steps follow it, so compact moves
the whole rhythm of a screen rather than its rows alone. It is a choice on the
About screen, remembered in `localStorage`, and carried on the document element
as `data-density="compact"`, the way the theme is carried as a class.

**Motion marks a state change and nothing else**, at one duration, on colour,
background and border. The base layer already stops every animation and
transition under `prefers-reduced-motion`
(<https://www.w3.org/TR/WCAG22/#animation-from-interactions>).

**Absence is a mark, not a sentence.** A fact the server did not state is an
em dash carrying its explanation in `title`. Eleven cells each saying "Not
loaded from an artifact" made absence the densest thing on the screen.

`src/styles.rs` is the vocabulary built on the tokens: the class string for a
panel, a table cell, an input, a badge, a link. A pairing that more than one
screen paints lives there and is measured once, which is what lets the WCAG
pass in `e2e/tests/it/accessibility.rs` cover every pairing the viewer draws.

**The rule is checked, not remembered.**
`scripts/checks/viewer-tokens.sh` runs as its own CI job and fails on a palette
entry, a per-element `dark:` variant, a raw type size, a spacing value off the
scale, or an arbitrary value in brackets, anywhere under `app/ferroterm-viewer/src`.

### Less prose, not more

Three long tables and a lead paragraph over every one of them is not a design.
A heading that says what a section is does not need a sentence saying it again,
and a figure a reader is deciding against should be the first thing they meet,
not the last. Each section of the About screen's evidence pane opens with the
one line it exists to say and keeps its table one press away; the runners hide
every parameter's explanation behind one switch, in the accessibility tree
throughout so a screen reader still reads it
(<https://www.w3.org/WAI/WCAG22/Techniques/css/C7>).

## 9. Screen inventory

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

| Screen | Route | Reads |
|---|---|---|
| Shell | all | `GET /health`; `GET /{v}/metadata` per version for the switcher; theme, density, language and page size from `localStorage` |
| Find | `/ui/find` | nothing until it offers: the typed string is read by its shape, and the offers are gated on `GET /{v}/metadata` |
| Overview | `/ui` | `GET /{v}/metadata?mode=terminology`, as one table, one row per served version |
| Code system | `/ui/systems/:url` | the same capability statement, plus `GET /{v}/CodeSystem?url=` for the published resource |
| Concept browser | `/ui/browse` | search through `ValueSet/$expand` with `filter`; the concept through `CodeSystem/$lookup`; the hierarchy through `$expand` over the version's declared child filter |
| Expand | `/ui/expand` | `ValueSet/$expand` by `url`, with `filter`, `count`, `offset`, `displayLanguage`, `activeOnly`, `includeDesignations` |
| Validate and subsume | `/ui/validate` | `CodeSystem/$validate-code`, `ValueSet/$validate-code`, `CodeSystem/$subsumes` |
| Translate | `/ui/translate` | `ConceptMap/$translate`, with the map, the source system, the code, and the target |
| Value sets | `/ui/valuesets` | `GET /{v}/ValueSet` search and read, with a link into the expansion runner |
| Concept maps | `/ui/conceptmaps` | `GET /{v}/ConceptMap` search and read, with a link into the translate runner |
| About this server | `/ui/about` | three panes: the four `CapabilityStatement`s side by side, the committed conformance and benchmark figures, and the per-viewer preferences |

`/ui/versions`, `/ui/evidence` and `/ui/settings` were screens of their own and
now redirect to the About pane they named, so a link written before the merge
still opens what it pointed at.

**A command bar sits above every screen.** One field. What a reader types is
read by its shape alone, before any request: a scheme with something after it
is a canonical, a short run with no space is a code, anything else is a phrase.
The offers are gated on what the root's `CapabilityStatement` declares, and
each one is an address the reader could have typed. It is a form rather than a
listbox that suggests as you type, so every offer is a real link and the
keyboard contract is the browser's own.

**The screens are reached from a left sidebar, in four labelled groups.**
Explore reads what the server holds, Run asks it something, Publish lists what
it publishes, About answers questions about the server rather than about a
code. Each group is a `const` table read in render order, so the order is data
and one function draws every entry, and each group's label is its list's
accessible name through `aria-labelledby`. Below the `md` breakpoint the
sidebar is hidden until the top bar's toggle opens it.

The top bar keeps what is true of every screen at once: the command bar, the
FHIR version switcher, the health chip, and the theme toggle. The switcher
changes which root every screen reads from, so it is a lens over the whole
viewer rather than a place to go, and that is why it stays out of the sidebar.

**A runner puts its answer in its own column.** Above the large breakpoint the
form takes a fixed column and the answer takes the rest, so a parameter that
lengthens the form never pushes the answer down the page; below it the two
stack, form first. A refusal renders where the answer would, as the server's
own `OperationOutcome`.

**A run is remembered in this browser.** Every runner puts its parameters in
the address, so a run is already a URL and the list holds links and nothing
else: a remembered run is re-run when a reader returns to it and can never show
a stale answer beside a live one. Twelve are kept, in `localStorage` alone.

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

## 10. What ports from FerroEHR, and what does not

FerroEHR's viewer is 52,796 lines of Rust over 23 screens for a clinical data
repository: sessions, OIDC, EHRs, compositions, templates, AQL, subscriptions,
audit. It shares no domain with a terminology browser. The split is therefore
sharp, and it favours porting the discipline rather than the code.

### Ports, adapted

| From FerroEHR | To FerroTERM | Adaptation |
|---|---|---|
| `.claude/rules/leptos-ui.md` | `.claude/rules/leptos-ui.md` | rewritten for CSR: the `ssr`/`hydrate` split, server functions, `<ActionForm>`, SSR modes, and progressive enhancement are gone; `LocalResource`, the fetch client, and the FHIR boundary replace them |
| `.claude/agent-memory/leptos-reviewer/` (17 hazards) | the same path | 16 ported and classified against CSR (§11); one dropped as tool-specific |
| `.claude/agents/leptos-reviewer.md` | the same path | the review priorities re-ordered around the FHIR boundary and bundle size |
| `.claude/agents/ui-implementer.md` | the same path | the gate list re-pointed at Trunk and the wasm target |
| `.claude/skills/leptos-lookup/SKILL.md` | the same path | unchanged in method; the cache path and the CSR chapters differ |
| `.claude/skills/ui-gates/SKILL.md` | the same path | the battery rewritten for one target and `trunk build` |
| `docker/viewer/Dockerfile` | `.github/workflows/release-build.yml` | **the reasoning ports; the commands and the location do not.** `docker/Dockerfile` compiles nothing, it copies binaries `release-build.yml` already built and attested, so the Trunk build belongs in that workflow, ahead of the `cargo auditable build` that embeds the bundle. What transfers is the record of why `cargo-chef` was removed (workspace members cannot survive the COPY boundary, so every source change recompiled the whole graph anyway) and how BuildKit cache mounts with `sharing=locked` bound peak memory. Its `cargo-leptos` invocation does not |
| the CI lane shapes in `build-image.yml`, `release-build.yml`, `ui-e2e-published.yml` | `ci.yml`, `release-build.yml`, `release-image.yml` | the shapes transfer: a wasm clippy pass, a formatter pass, a bundle build, an E2E job against a published artifact. The commands are Trunk's |
| `style/tailwind.css` token layer, `theme.rs` | the same shape | the token layer is a starting point, recoloured for FerroTERM. The component-library brand ramp does not port: there is no component library (§3) |
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

**Every screen in §9 is written from scratch.** The estimate that a large
amount ports is right about the scaffolding and the discipline, and wrong
about the screens.

## 11. The recorded hazards, re-audited under CSR

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
| `thaw-hydration-hazards` | moot | the hydration mismatch went with SSR and the widgets went with `thaw` (§3). The method survives and generalizes: read a widget's source before trusting what it renders for `id` and `for` |
| `w2-confirmed-good-patterns` | changed | the auth-guard half is moot. The `.into_any()` section erasure still applies, because the rustc layout-recursion limit is a codegen fact; so do the fixed-size-integer and theme-effect findings |
| `tabbed-screen-pattern` | changed | always-mounted bodies were a hydration-stability device and are no longer required. Gating each tab's resource on the active tab still applies and is now the whole point |
| `redirect-path-must-be-percent-encoded` | changed | the `leptos_axum::redirect` panic is gone with the server. The rule survives and grows teeth: system URIs, ECL, and concept ids all land in FHIR request URLs, and every one is percent-encoded |
| `no-js-journeys-must-click` | changed | there is no no-JavaScript journey. The residue is the review rule: an assertion on page source proves nothing, so a journey drives the real widget |
| `thaw-field-random-id` | moot | `thaw::Field` mints a `Uuid::new_v4()` id at setup, and the viewer no longer depends on `thaw`. Kept: an explicit stable id is still wanted for label association and for E2E selectors |
| `thaw-input-name-forwarding-ok` | moot | it recorded that `thaw::Input` forwards `name` so an `<ActionForm>` submits without WebAssembly. There is no `<ActionForm>`, no no-JavaScript path, and no `thaw` |
| `chartistry-chart-hydration` | moot | the chart self-gates on a client measurement, which was the hydration answer. The residue is an E2E fact: the chart renders a placeholder until its container is measured, so a journey waits on the drawn chart |
| `redirect-needs-ssrmode-async` | moot | `SsrMode` does not exist under CSR |
| `seed-once-form-idiom` | changed | the hydration half is moot. The refetch-versus-edit-in-progress half stands: a form seeded from a resource must not overwrite what the reader is typing |
| `default-style-guard-untracked-blindspot` | not ported | it describes `scripts/checks/default-style.sh`, a FerroEHR script with no counterpart here |

## 12. The build checklist

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

## 13. What CI and CD gain

The additions, recorded in `docs/ci-cd.md` as they land. Everything below is
built except the `ui-e2e` job:

- **`ci.yml`, a `viewer` job.** `cargo fmt` and `leptosfmt --check` over the
  crate, `cargo clippy --target wasm32-unknown-unknown --all-features -D
  warnings`, `cargo nextest run -p ferroterm-viewer` for the component-free
  logic, and `trunk build --release --locked`. The wasm target is the gate
  that matters: it is the only place a dependency that cannot compile for the
  browser shows up.
- **A recorded bundle size.** The compressed `.wasm` size is written to a
  committed file and compared on every build, the same shape
  `scripts/checks/bench-bars.sh` already uses for latency: a claim that never
  moves to match a slower build. The basis is below.
- **`ui-e2e`, a merge gate.** `thirtyfour` driving headless Chromium against a
  container built from the same Dockerfile, with the journeys as plain
  `#[tokio::test]`s. Every journey fails on a browser console error. Rust
  only: Playwright is JavaScript and the no-JavaScript-authored mandate covers
  the test suite. The journeys live in `e2e/`, a crate the root manifest
  excludes: `thirtyfour` depends on `serde_json` with `preserve_order`, and
  cargo unifies features across one invocation
  (<https://doc.rust-lang.org/cargo/reference/features.html#feature-unification>),
  so holding it inside the workspace would make the test run write FHIR JSON
  in a key order the shipped server does not.
- **The release lane.** The bundle is architecture-independent, so it is built
  once and embedded into each per-architecture binary before
  `release-build.yml` attests it. No new image, no new attestation subject,
  and no change to the SLSA Build L3 shape: the bundle becomes part of the
  binary the existing lane already signs.
- **`docker/Dockerfile` is unchanged.** It compiles nothing today: it copies
  the two binaries `release-build.yml` built and attested. The bundle rides
  inside the server binary, so the image gains no stage, no file, and no new
  `hadolint` surface.

### The bundle bar

**What it measures.** The whole `.wasm` a reader downloads, gzipped: the
viewer's own code and every dependency in it, together. There is no separate
bar for project code. A reader downloads one file and waits for one file, so
one artifact is what the claim is about, and a dependency the viewer chose is
the viewer's weight. `app/ferroterm-viewer/bundle-size.json` holds the bars for
the three assets (wasm, JS bootstrap, CSS) and `scripts/checks/bundle-size.sh`
checks the build in `dist/`, in the `viewer` CI job.

**Two numbers, because one cannot do both jobs.** A single absolute total on an
artifact that legitimately grows has to be raised every time it grows, and a
number edited every fortnight has stopped being a claim. One screen costs about
27,000 gzipped bytes (the code system detail screen, measured 2026-09-07) and
nine more screens are to come, so any total tight enough to catch a regression
today is breached by honest work next week. The two jobs are split:

- **`max_gzip_bytes` is the ceiling**, the claim about what the finished viewer
  costs a reader. It is set from arithmetic over the screens still to land
  rather than from today's build, so it does not move screen by screen.
  **540,000 bytes** (528 KiB). It was 470,000, derived when one screen had been
  measured and the arithmetic assumed nine more like it at 27,043. Five screens
  in, the mean is 39,737 and the largest is 68,519, and the same arithmetic on
  real figures runs past 470,000 before the last screen lands:

  | | gzipped |
  |---|---|
  | main with five screens | 331,893 |
  | the concept browser, measured | 68,519 |
  | the four versions side by side, the evidence screen, the accessibility pass | about 90,000 together |
  | projected finish | about 490,000 |

  540,000 carried that with room for the one unmeasured thing ahead:
  `leptos-chartistry`, which the evidence screen (#414) was expected to pull
  in and which nothing had weighed.

  **That guess is now resolved.** Section 3 records the measurement: the crate
  costs 132,228 gzipped bytes, so the evidence screen refused it and draws its
  figures as tables with a proportional bar in the cell. The last screen of the
  inventory has now landed, so the ceiling no longer carries an unknown, and
  the arithmetic that set it can be replaced by measurement. The owner sets the
  number; the figures a re-derivation needs are the recorded
  `measured_gzip_bytes` on `main` and the growth of the accessibility pass
  (#415), which is a sweep over screens that already exist rather than a new
  one.

  The viewer is an operator tool served from the host it browses, its assets are
  content-hashed and cached immutably, and a reader downloads it once.
- **`max_growth_gzip_bytes` is the per-change budget**, and it is the gate that
  catches a surprise. **70,000 bytes** for the wasm.

  This is the third setting and the first taken from a sample worth the name.
  32,000 was chosen between two landmarks, a screen at 27,043 and the `thaw`
  dependency at 36,894. 45,000 followed when the expansion runner came in at
  40,940 and showed that a legitimate screen could outweigh the dependency the
  number was sized to catch. Five screens are now measured and they span a
  factor of two and a half:

  | screen | gzipped growth |
  |---|---|
  | code system detail | 26,736 |
  | value sets | 27,089 |
  | concept maps | 35,403 |
  | expansion runner | 40,940 |
  | concept browser | 68,519 |

  A single number cannot separate a screen from a library across that range, so
  it no longer tries. That job belongs to instruments that can do it: a
  dependency arrives in `Cargo.toml` and `Cargo.lock` where it is read rather
  than weighed, `viewer-boundary.sh` refuses a workspace crate outright, and
  `max_gzip_bytes` bounds the total whatever the increments were. 70,000 admits
  the largest screen measured and still refuses an increment nobody intended.
  The guard prints the delta on every run, passing or not, so growth is visible
  before it is a breach.

  The concept browser is the evidence that the range is real rather than
  slack. Its author took the §13 path first: a lone `format!` over an `f64` was
  pulling `core::fmt::float` into a bundle with no other float, and removing it
  with two other reductions bought 8,962 bytes. With the whole tree section
  stubbed out the screen still cost 49,533, because it is a search, a concept
  detail, a hierarchy, capability gating and a language picker over four
  requests.

**A change cannot make its own build green by editing a number.** The guard
reads `measured_gzip_bytes` **out of git at the merge base**, not out of the
branch under test, so the figure a change is judged against is not a figure the
change can write. Editing `measured_gzip_bytes` in a branch does nothing at
all. The one knob left in the tree is `max_growth_gzip_bytes`, and it is a
rate: raising it raises it for every future change at once, in a one-line diff
a reviewer sees, rather than nudging a level by exactly one branch's overage.

That leaves a bookkeeping duty. A change that grows the bundle records its own
CI figure as `measured_gzip_bytes`, which moves the baseline forward by one
change. Skipping it does not help the change that skipped it; it charges the
next change for both, and the guard says so when it fails. The figure is CI's,
on x86_64 Linux, because another host emits slightly different bytes (an
aarch64-apple-darwin build runs about 550 over), and the budget is three orders
of magnitude wider than that spread.

**What a slice does when it breaches**, in order, and never by starting at the
end:

1. **Measure the composition first.** `twiggy top -f csv` over
   `target/wasm32-unknown-unknown/wasm-release/ferroterm-viewer.wasm`, the
   module before `wasm-opt` strips the name section, aggregated by the crate
   that owns each item. Then rebuild without the suspect and compare the
   gzipped `dist/` figure. An unmeasured claim about what is heavy is worth
   nothing: the claim this viewer carried for a release, that `chrono` and
   `icondata_ai` were most of its weight, was false by 100%. The same held for
   a 1,599-icon pack: building the icon set both ways put `icondata_lu` at
   1,737 gzipped bytes over hand-written SVG, not the hundreds of kilobytes a
   whole pack would be (§3).
2. **Remove weight the viewer does not use.** A dependency whose surface is far
   larger than the use is the first place to look, and the largest single lever
   found so far.
3. **Cut monomorphization.** Erase sections with `.into_any()` and factor a
   concrete inner function out of a generic one. Removing `thaw` took 35,722
   bytes of `reactive_graph` instantiations with it, more than `thaw`'s own
   code.
4. **Only then re-adjudicate a number**, with the measurement in the commit
   message and the alternatives ruled out recorded here. A slice never raises
   a bar to make its own build green. A screen that genuinely costs a screen
   records its figure and moves on; that is the budget working, not a breach.

**The measurement, 2026-09-07** (`twiggy` 0.8.0 over the pre-`wasm-opt` module,
plus an A/B rebuild; the gzipped figures are the CI `viewer` job's, on x86_64
Linux):

| Suspect | In the 238,619-byte bundle |
|---|---|
| `chrono` | 0 bytes. No item in the module is owned by it |
| `icondata_ai` | 0 bytes. 2,265 bytes of `reactive_graph` glue were monomorphized over `Option<&icondata_core::IconData>`, `thaw::Button`'s icon prop type |
| `palette`, `pure-rust-locales`, `num-traits` | 0 bytes each |
| `thaw` itself | 34,997 bytes of its own code, and 36,894 gzipped bytes of the shipped bundle once its induced instantiations and its 20,399 bytes of colour-token data are counted |

Link-time optimization with `--gc-sections` already removes a dependency the
viewer never calls; what it cannot remove is a library the viewer does call,
for three widgets out of a hundred. Removing `thaw` took the wasm from 238,619
to 201,725 gzipped bytes, 15.5% of the bundle, against the 1,381 bytes of
headroom the overview screen had left. Nine screens now have 38,275.

Three alternatives were ruled out.

- **Patching or vendoring `thaw` to drop `chrono` and `icondata_ai`** would
  have bought nothing, because neither was in the bundle.
- **Gating a pull request on the project's own code, with dependency weight
  tracked separately.** It gates the wrong half in both directions: the
  project's own code is the half that grows by a screen at a time, so that bar
  needs raising nine more times, while the dependency half that was 15.5% of
  the download stops being counted at all. The per-change budget catches a
  dependency and a screen with one number, because it measures what a reader
  actually waits for.
- **Keeping a single tight total and raising it as screens land.** That is nine
  more re-adjudications, each one a change making its own build green, which is
  the failure this section exists to prevent.

## 14. Sources

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

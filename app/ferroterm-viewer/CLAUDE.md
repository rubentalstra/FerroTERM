# CLAUDE.md (`app/ferroterm-viewer`)

The Leptos web UI: a client-side-rendered FHIR terminology browser, compiled to
WebAssembly, built by Trunk, and served by the one FerroTERM binary as static
assets. The design is `docs/viewer.md`; the enforceable discipline is
**`.claude/rules/leptos-ui.md`**, which loads whenever a file here is read.
Every section of that file applies, and nothing below repeats it.

Three properties decide most questions in this crate:

- **The crate depends on no workspace crate.** Every request goes through
  `crate::fhir` over `gloo-net`, same-origin, with the base derived from the
  page. `scripts/checks/viewer-boundary.sh` walks the resolved closure and
  fails on an edge into the engine.
- **No code system is a special case.** Every screen renders from
  `TerminologyCapabilities` and the operations. The test
  `pages::tests::no_screen_names_a_code_system` refuses a canonical written
  into a screen.
- **Zero hand-written JavaScript.** The only JavaScript in the product is the
  `wasm-bindgen` bootstrap the toolchain generates.

## Two bundles from one crate

The crate builds twice, and the server serves both trees (`docs/viewer.md`
§2):

| Bundle | Build | Served at | Carries |
|---|---|---|---|
| reader | `trunk build --release --locked` | `/ui` | every reading screen |
| editor | the same, plus `--features editor --dist dist-editor --public-url /ui/editor/` | `/ui/editor` | the reading screens and the authoring screens |

`crate::routes::UI_BASE` is the router base, and it is the one thing the
`editor` feature changes outside the authoring code, so every link in either
bundle stays inside its own tree. A reader who never signs in downloads no
editor byte.

Two consequences for anything written here:

- **An address in a test is written against `UI_BASE`**, never as a `/ui`
  literal: the same test runs in both feature sets, and the CI `viewer` job
  runs it in both.
- **Each bundle has its own size bars**, `bundle-size.json` and
  `bundle-size-editor.json`, checked by `scripts/checks/bundle-size.sh` over
  `dist/` and `dist-editor/`. The reader's ceiling is never raised to make room
  for the editor.

The authoring screens are `src/pages/editor.rs` over the form model in
`src/editor.rs` and the reads in `src/fhir/authoring.rs`, all three behind the
feature. The model is plain values and plain functions, so the rules it
encodes (what a lifecycle transition writes, what a save sends) are pinned by
ordinary unit tests rather than by driving a browser.

## Gates

`/ui-gates` runs the battery over both bundles: `cargo fmt` and `leptosfmt`,
`cargo clippy --target wasm32-unknown-unknown -- -D warnings` with and without
the feature, `cargo nextest run -p ferroterm-viewer` in both, both Trunk
release builds, and both bundle-size checks. The browser journeys are
`scripts/ui-e2e.sh`, and CI's `ui-e2e` job gates the merge whatever ran
locally.

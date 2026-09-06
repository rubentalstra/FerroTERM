---
name: ui-gates
description: >
  Runs the full viewer quality-gate battery for app/ferroterm-viewer:
  formatting, clippy on the wasm32 target, nextest, a Trunk release build,
  and the recorded bundle size. Use before committing any viewer change, when
  the user asks to "check the UI", or as the done-gate a ui-implementer task
  must pass.
allowed-tools: Bash, Read, Grep, Glob
---

# /ui-gates

Run every gate the viewer must pass (`.claude/rules/leptos-ui.md` §10). Stop and
report on the first hard failure; run the cheap gates first.

## Preconditions

- `app/ferroterm-viewer` must exist. If it does not, say so and stop: the
  viewer is designed in `docs/viewer.md` and tracked under issue #366.
- Tooling presence, reported rather than silently installed:
  `rustup target list --installed | grep wasm32` (add with
  `rustup target add wasm32-unknown-unknown`), `trunk --version`,
  `leptosfmt --version`. Ask before installing anything.
- Shared `./target`, no ad-hoc `RUSTFLAGS`, no flag variation between runs.

## The battery, in order

```bash
# 1. Formatting. tests/ carries view! macros too, so leptosfmt covers both.
cargo fmt -p ferroterm-viewer --check
leptosfmt --check app/ferroterm-viewer/src app/ferroterm-viewer/tests

# 2. Clippy on the target the crate actually ships to. This is the gate that
#    catches a dependency that cannot compile for the browser.
cargo clippy -p ferroterm-viewer --target wasm32-unknown-unknown \
  --all-features --all-targets -- -D warnings

# 3. Tests: the component-free logic (URL building, OperationOutcome
#    flattening, capability reading, paging arithmetic, the tree model).
cargo nextest run -p ferroterm-viewer

# 4. The full bundle, only when the change touches the build surface
#    (Cargo.toml, index.html, Trunk.toml, styles, assets); otherwise report it
#    skipped with the reason. --locked so the build cannot re-resolve.
(cd app/ferroterm-viewer && trunk build --release --locked)

# 5. The recorded bundle size. A claim never moves to match a fatter build.
bash scripts/checks/bundle-size.sh

# 6. E2E journeys (merge-gating in CI; local needs Docker): thirtyfour over
#    WebDriver against the built image.
bash scripts/ui-e2e.sh
```

Stage 6 locally: the script builds the image `docker/Dockerfile` describes, and
that image stages linux binaries, so the managed mode needs Docker
(`docker info`) and a Linux host. Anywhere else, report `SKIPPED(reason)` and
state explicitly that CI's `ui-e2e` job runs it and gates the merge, so a skip
here is not a pass. To drive a server you started yourself, pass both
`--base-url` and `--webdriver`; the address must be one the browser can reach,
which for a browser in a container is `host.docker.internal`, never
`127.0.0.1`.

Read the crate's `Cargo.toml` and `Trunk.toml` before running: the feature
names and the `dist` path are the convention, not a guess.

## Report

One line per gate: PASS, FAIL, or SKIPPED(reason), with the failing output
excerpted verbatim on a failure. Never mark a gate green you did not run. A
FAIL is never fixed by weakening the gate (removing a lint, deleting a test,
dropping the wasm pass, raising the size bar): fix the code.

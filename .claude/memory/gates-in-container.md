---
name: gates-in-container
description: "fmt, clippy, doc, and the check scripts run on the host CLI as always; only the test run (cargo nextest, which execs freshly built binaries) runs inside the pinned rust:1.98-bookworm container, because macOS Gatekeeper stalls every fresh unsigned binary about 4 minutes; owner-decided 2026-09-04"
metadata: 
  node_type: memory
  type: feedback
  originSessionId: f1d07f84-66e2-401d-8ebd-76aea8bebf4f
  modified: 2026-09-04T11:57:53.198Z
---

Keep `cargo fmt --check`, `cargo clippy -D warnings`, `cargo doc`, `scripts/checks/versions.sh`, and `scripts/checks/crate-version-guard.sh` on the host CLI. Run only `cargo nextest run --workspace --locked` inside the repo's pinned Rust image (the `rust:1.98-bookworm` digest from `bench/Dockerfile`): checkout bind-mounted at `/src`, `CARGO_TARGET_DIR=/target` on the named volume `ferroterm-gates-target`, the registry on `ferroterm-gates-cargo`, cargo-nextest from `https://get.nexte.st/latest/linux-arm`.

**Why:** on the owner's Mac (Darwin 25.5, 2026-09-04) macOS Gatekeeper (`syspolicyd`) assesses every freshly built unsigned binary on exec, so each nextest `--list` sat at 0% CPU for about 4 minutes and a workspace run would take hours. Compiling (fmt, clippy, doc) execs no fresh binary and is fast on the host. The owner refused running the binaries unsandboxed and said "gates like fmt and clippy should [not] be in the docker, keep it in the cli"; the container is for the test binaries only. `cargo run -p fhir-codegen -- emit` execs one fresh binary and takes the stall once; accept it on the host.

**How to apply:** one `docker run --rm` with the mounts above for nextest, in the background, output to a scratchpad log; the host chain runs concurrently (separate target dirs). Do not edit manifests while either cargo run is in flight. See [[repo-merge-gates]].

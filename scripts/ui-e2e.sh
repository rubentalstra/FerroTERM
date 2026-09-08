#!/usr/bin/env bash
# SPDX-License-Identifier: BUSL-1.1
# The viewer's browser journeys: a headless Chromium driving the bundle a real
# FerroTERM serves, over WebDriver (https://www.w3.org/TR/webdriver2/).
#
#   scripts/ui-e2e.sh
#   scripts/ui-e2e.sh --base-url URL --webdriver URL
#   scripts/ui-e2e.sh --docs-shots
#
# The battery must not change a tracked file. The documentation capture pass is
# the one exception, and it runs only when --docs-shots asks for it: it drives
# the same deployment and writes one PNG per viewer screen into
# website/book/src/operate/img/viewer, which the book embeds. An ordinary run,
# on a pull request or on a laptop, never rewrites an image.
#
# Without arguments the script owns everything it drives. It builds the bundle
# with Trunk, builds the server with the bundle inside it, builds the image
# from docker/Dockerfile the way the release lane stages it, and runs that
# image and a pinned Chromium beside it on a private container network. The
# journeys then run against the image the project actually ships.
#
# With --base-url and --webdriver it builds and starts nothing and drives what
# you already have. Both are required together, because a browser that cannot
# reach the address is a red lane with no defect behind it: a browser in a
# container reaches a server on the host as host.docker.internal, not as
# 127.0.0.1. A server started that way serves the journeys' own fixture only if
# it was pointed at e2e/fixtures/codesystems with FERROTERM_CODESYSTEMS, which
# the tree journey needs and the capture pass needs on every screen.
#
# FERROTERM_UI_E2E_SHOTS_DIR sends the capture somewhere other than the book,
# for looking at a shot without touching the checkout.
#
# A journey that fails writes what it was looking at into
# target/ui-e2e-failures: a screenshot and the whole document, one pair per
# failed wait. A passing run leaves the directory empty, and the ui-e2e CI job
# uploads it as an artifact only when the run failed.
#
# A capture renders the FHIR base the viewer read, so whatever address the
# browser used lands in the images. The managed mode gives the server the fixed
# network alias below; a manual capture reproduces the same images by serving on
# 8080 and starting the browser container with
# --add-host ferroterm:host-gateway, then passing --base-url http://ferroterm:8080.
#
# The image stages linux binaries, so the managed mode needs a Linux host.
# Anywhere else it says so and stops rather than reporting a lane it did not
# run.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# The browser, pinned by index digest so every run drives the same Chromium and
# the same chromedriver. Selenium publishes the pair in one image and keeps
# them in step; the tag records which pair this digest is.
readonly BROWSER_IMAGE="selenium/standalone-chromium:4.48.0-20260905@sha256:fcf9eef47b9546a2252937481a8298ce0958d20c9d91e040d480184e80b41c76"

# The tag the locally built server image is loaded under. It is never pushed.
readonly SERVER_IMAGE="ferroterm-ui-e2e:local"

# The name the browser addresses the server by on the private network.
readonly SERVER_HOST="ferroterm"

# Seconds to wait for the server container to answer /health, and for the
# browser container to report itself ready.
readonly READY_TIMEOUT=120

# How many journeys drive the browser at once. The Selenium image allows one
# session per container by default, so a second journey would sit in the grid's
# new-session queue until the first quit; the image's own README says to raise
# the ceiling with SE_NODE_MAX_SESSIONS plus SE_NODE_OVERRIDE_MAX_SESSIONS, and
# not to exceed the available processors
# (https://github.com/SeleniumHQ/docker-selenium).
# Four is the ubuntu-latest runner's processor count. The same number is the
# test-thread count below, so the journeys never ask for a session the browser
# cannot open. A browser you started yourself has its own ceiling; give it at
# least this many or the journeys queue behind each other.
readonly BROWSER_SESSIONS=4

# Where a failing journey writes its evidence: a screenshot and the whole
# document. The ui-e2e CI job uploads this directory when the run fails, and a
# passing run leaves it empty, so nothing is attached for a green lane.
readonly FAILURES_DIR="$root/target/ui-e2e-failures"

base_url=""
webdriver=""
docs_shots=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --base-url) base_url=$2; shift 2 ;;
    --webdriver) webdriver=$2; shift 2 ;;
    --docs-shots) docs_shots=1; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if { [[ -n "$base_url" ]] && [[ -z "$webdriver" ]]; } ||
   { [[ -z "$base_url" ]] && [[ -n "$webdriver" ]]; }; then
  echo "ui-e2e: --base-url and --webdriver are given together or not at all" >&2
  exit 2
fi

need() {
  command -v "$1" >/dev/null 2>&1 ||
    { echo "ui-e2e: $1 is not installed; $2" >&2; exit 1; }
}

need cargo "install the toolchain in rust-toolchain.toml"
need cargo-nextest "cargo install cargo-nextest --locked"
need curl "the readiness probes ask the server and the browser whether they answer"

# Whether anything is listening on a local port, so a published container port
# never lands on a socket another process already holds.
port_taken() {
  (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null || return 1
  exec 3<&-
  return 0
}

# The first free port at or above $1.
free_port() {
  local candidate
  for candidate in $(seq "$1" "$(($1 + 40))"); do
    if ! port_taken "$candidate"; then
      echo "$candidate"
      return 0
    fi
  done
  echo "ui-e2e: no free port in $1..$(($1 + 40))" >&2
  return 1
}

network=""
server=""
browser=""
cleanup() {
  [[ -z "$browser" ]] || docker rm -f "$browser" >/dev/null 2>&1 || true
  [[ -z "$server" ]] || docker rm -f "$server" >/dev/null 2>&1 || true
  [[ -z "$network" ]] || docker network rm "$network" >/dev/null 2>&1 || true
}

if [[ -z "$base_url" ]]; then
  need docker "the managed mode runs the server and the browser as containers"
  need trunk "the pin is in docs/VERSIONS.md; the bundle is what the journeys drive"
  docker info >/dev/null 2>&1 ||
    { echo "ui-e2e: docker is installed but not running" >&2; exit 1; }
  if [[ "$(uname -s)" != Linux ]]; then
    echo "ui-e2e: docker/Dockerfile stages linux binaries, which this host cannot build." >&2
    echo "  Run this on Linux (the ui-e2e CI job does), or start a server and a" >&2
    echo "  WebDriver yourself and pass --base-url and --webdriver." >&2
    exit 1
  fi

  case "$(uname -m)" in
    x86_64 | amd64) arch=amd64 ;;
    aarch64 | arm64) arch=arm64 ;;
    *) echo "ui-e2e: no image architecture for $(uname -m)" >&2; exit 1 ;;
  esac

  echo "== the viewer bundle"
  # `locked = true` in Trunk.toml already refuses a stale lock file; the flag
  # says so at the call site too.
  (cd app/ferroterm-viewer && trunk build --release --locked)

  # The image base is distroless/static, which carries no dynamic loader, so a
  # glibc-linked binary copies in and then fails `exec` with "no such file or
  # directory". release-image.yml maps the image architecture to a musl triple
  # for the same reason; this mirrors it rather than inventing a second answer.
  case "$arch" in
    amd64) target=x86_64-unknown-linux-musl ;;
    arm64) target=aarch64-unknown-linux-musl ;;
    *) echo "ui-e2e: no musl target for $arch" >&2; exit 1 ;;
  esac
  if ! rustup target list --installed | grep -qx "$target"; then
    echo "== adding the $target target"
    rustup target add "$target"
  fi

  echo "== the server, with the bundle inside it"
  # Naming the bundle directory is what makes a missing bundle fail the build:
  # the server's build script refuses when the variable names a directory that
  # does not read, and only warns when it falls back to the default. Without it
  # the journeys would drive a viewer-less binary and nothing would say so.
  FERROTERM_UI_BUNDLE="$root/app/ferroterm-viewer/dist" \
    cargo build --release --locked --target "$target" -p ferroterm-server --features ui
  cargo build --release --locked --target "$target" -p ferroterm-build

  echo "== the image, staged the way the release lane stages it"
  # .dockerignore admits nothing but dist/, so the repository root is a cheap
  # build context even with a populated target/.
  rm -rf "dist/linux/$arch"
  mkdir -p "dist/linux/$arch"
  cp "target/$target/release/ferroterm" "target/$target/release/ferroterm-build" \
    "dist/linux/$arch/"
  docker build --file docker/Dockerfile --tag "$SERVER_IMAGE" \
    --build-arg "TARGETOS=linux" --build-arg "TARGETARCH=$arch" .

  run="ferroterm-ui-e2e-$$"
  network="$run-net"
  server="$run-server"
  browser="$run-browser"
  trap cleanup EXIT
  docker network create "$network" >/dev/null

  # The server port is published as well as networked: the readiness probe and
  # anyone debugging a failure reach it from the host, while the browser
  # resolves the container by name on the private network.
  server_port="$(free_port 8140)"
  # The registry systems the server ships with are flat, so a deployment
  # carrying only them declares no child-of filter operator and the viewer's
  # taxonomy tree has nothing to draw. e2e/fixtures/codesystems holds one
  # shaped, synthetic CodeSystem resource so the tree journey drives a real
  # hierarchy; it is mounted read-only and never baked into the image.
  # The alias is what the browser addresses the server by, and it is fixed
  # while the container name carries this run's pid. A screenshot the capture
  # pass takes renders the FHIR base it read, so an address with a pid in it
  # would change every image on every run.
  docker run --detach --name "$server" --network "$network" \
    --network-alias "$SERVER_HOST" \
    --env FERROTERM_UI=on --env FERROTERM_LOG_FORMAT=json \
    --env FERROTERM_CODESYSTEMS=/fixtures/codesystems \
    --volume "$root/e2e/fixtures/codesystems:/fixtures/codesystems:ro" \
    --publish "127.0.0.1:$server_port:8080" "$SERVER_IMAGE" >/dev/null
  echo "== waiting for the server on 127.0.0.1:$server_port"
  ready=""
  for _ in $(seq 1 "$((READY_TIMEOUT * 5))"); do
    if [[ "$(docker inspect -f '{{.State.Running}}' "$server" 2>/dev/null)" != true ]]; then
      echo "ui-e2e: the server container exited before it was ready" >&2
      docker logs "$server" >&2 || true
      exit 1
    fi
    if curl -sf "http://127.0.0.1:$server_port/health" >/dev/null 2>&1; then
      ready=1
      break
    fi
    sleep 0.2
  done
  if [[ -z "$ready" ]]; then
    echo "ui-e2e: the server did not answer /health within ${READY_TIMEOUT}s" >&2
    docker logs "$server" >&2 || true
    exit 1
  fi
  # A binary built without a bundle serves no /ui route, and the journeys would
  # then fail on a missing element rather than on the missing bundle.
  curl -sf "http://127.0.0.1:$server_port/ui/" >/dev/null 2>&1 || {
    echo "ui-e2e: the server serves no /ui, so this binary carries no viewer bundle" >&2
    docker logs "$server" >&2 || true
    exit 1
  }

  # Chromium needs more than the default 64 MB /dev/shm; without this it
  # crashes on a page of any size
  # (https://developer.chrome.com/docs/chromium/headless).
  webdriver_port="$(free_port 4444)"
  docker run --detach --name "$browser" --network "$network" --shm-size 2g \
    --env "SE_NODE_MAX_SESSIONS=$BROWSER_SESSIONS" \
    --env SE_NODE_OVERRIDE_MAX_SESSIONS=true \
    --publish "127.0.0.1:$webdriver_port:4444" "$BROWSER_IMAGE" >/dev/null
  echo "== waiting for the browser on 127.0.0.1:$webdriver_port"
  ready=""
  for _ in $(seq 1 "$((READY_TIMEOUT * 5))"); do
    if [[ "$(docker inspect -f '{{.State.Running}}' "$browser" 2>/dev/null)" != true ]]; then
      echo "ui-e2e: the browser container exited before it was ready" >&2
      docker logs "$browser" >&2 || true
      exit 1
    fi
    if curl -sf "http://127.0.0.1:$webdriver_port/status" 2>/dev/null | grep -q '"ready": *true'; then
      ready=1
      break
    fi
    sleep 0.2
  done
  if [[ -z "$ready" ]]; then
    echo "ui-e2e: the browser did not report itself ready within ${READY_TIMEOUT}s" >&2
    docker logs "$browser" >&2 || true
    exit 1
  fi

  base_url="http://$SERVER_HOST:8080"
  webdriver="http://127.0.0.1:$webdriver_port"
fi

# A previous run's evidence would be uploaded beside this run's, so the
# directory starts empty and is only written to by a journey that fails.
rm -rf "$FAILURES_DIR"
mkdir -p "$FAILURES_DIR"

echo "== the journeys, against $base_url through $webdriver"
# The journeys live outside the workspace, for the reason e2e/Cargo.toml
# records, so they are run by manifest path rather than by package.
#
# The capture pass is excluded by a nextest set difference, so it never runs
# beside the journeys and never writes an image nobody asked for
# (https://nexte.st/docs/filtersets/).
FERROTERM_UI_E2E_BASE_URL="$base_url" \
  FERROTERM_UI_E2E_WEBDRIVER="$webdriver" \
  FERROTERM_UI_E2E_FAILURES="$FAILURES_DIR" \
  cargo nextest run --manifest-path e2e/Cargo.toml --locked \
    --test-threads "$BROWSER_SESSIONS" \
    -E 'binary(it) - test(/^docs_shots::/)'

# The documentation capture pass, which is the one thing here that writes into
# the checkout. It runs after the journeys, so an image is only ever taken of a
# viewer the journeys have just found working.
if [[ -n "$docs_shots" ]]; then
  echo "== the documentation screenshots, into website/book/src/operate/img/viewer"
  FERROTERM_UI_E2E_BASE_URL="$base_url" \
    FERROTERM_UI_E2E_WEBDRIVER="$webdriver" \
    FERROTERM_UI_E2E_FAILURES="$FAILURES_DIR" \
    FERROTERM_UI_E2E_DOCS_SHOTS=1 \
    cargo nextest run --manifest-path e2e/Cargo.toml --locked \
      -E 'test(/^docs_shots::/)'
fi

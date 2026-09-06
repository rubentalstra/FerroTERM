#!/usr/bin/env bash
# SPDX-License-Identifier: BUSL-1.1
# The viewer's browser journeys: a headless Chromium driving the bundle a real
# FerroTERM serves, over WebDriver (https://www.w3.org/TR/webdriver2/).
#
#   scripts/ui-e2e.sh
#   scripts/ui-e2e.sh --base-url URL --webdriver URL
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
# 127.0.0.1.
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

# Seconds to wait for the server container to answer /health, and for the
# browser container to report itself ready.
readonly READY_TIMEOUT=120

base_url=""
webdriver=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --base-url) base_url=$2; shift 2 ;;
    --webdriver) webdriver=$2; shift 2 ;;
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

  echo "== the server, with the bundle inside it"
  # Naming the bundle directory is what makes a missing bundle fail the build:
  # the server's build script refuses when the variable names a directory that
  # does not read, and only warns when it falls back to the default. Without it
  # the journeys would drive a viewer-less binary and nothing would say so.
  FERROTERM_UI_BUNDLE="$root/app/ferroterm-viewer/dist" \
    cargo build --release --locked -p ferroterm-server --features ui
  cargo build --release --locked -p ferroterm-build

  echo "== the image, staged the way the release lane stages it"
  # .dockerignore admits nothing but dist/, so the repository root is a cheap
  # build context even with a populated target/.
  rm -rf "dist/linux/$arch"
  mkdir -p "dist/linux/$arch"
  cp target/release/ferroterm target/release/ferroterm-build "dist/linux/$arch/"
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
  docker run --detach --name "$server" --network "$network" \
    --env FERROTERM_UI=on --env FERROTERM_LOG_FORMAT=json \
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

  base_url="http://$server:8080"
  webdriver="http://127.0.0.1:$webdriver_port"
fi

echo "== the journeys, against $base_url through $webdriver"
# The journeys live outside the workspace, for the reason e2e/Cargo.toml
# records, so they are run by manifest path rather than by package.
FERROTERM_UI_E2E_BASE_URL="$base_url" \
  FERROTERM_UI_E2E_WEBDRIVER="$webdriver" \
  cargo nextest run --manifest-path e2e/Cargo.toml --locked

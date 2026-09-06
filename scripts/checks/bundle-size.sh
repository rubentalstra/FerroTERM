#!/usr/bin/env bash
# SPDX-License-Identifier: BUSL-1.1
# The viewer bundle every reader downloads, measured gzipped and compared to
# the claims in app/ferroterm-viewer/bundle-size.json.
#
#   scripts/checks/bundle-size.sh [--bars FILE] [--dist DIR]
#
# A bar is the claim the project makes, never a build's measurement: it does
# not move to match a fatter bundle. A breach is a dependency or a screen to
# justify, or a claim to re-adjudicate, never a bar to raise. Each bar records
# the figure the build last produced (`measured_gzip_bytes`), printed beside the
# live measurement so the headroom is visible.
#
# The bundle is built by `trunk build --release` from app/ferroterm-viewer; a
# missing dist/ SKIPS LOUDLY rather than failing, because no ordinary cargo
# gate builds it.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

bars=app/ferroterm-viewer/bundle-size.json
dist=app/ferroterm-viewer/dist

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bars) bars=$2; shift 2 ;;
    --dist) dist=$2; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ ! -f "$bars" ]]; then
  echo "bundle-size: no bars at $bars" >&2
  exit 2
fi

if [[ ! -d "$dist" ]]; then
  floor="$(sed -n 's/^trunk-version[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' app/ferroterm-viewer/Trunk.toml 2>/dev/null | head -n1)"
  echo "bundle-size: no bundle at $dist. SKIPPED, which is NOT a pass:"
  echo "  build it with 'cd app/ferroterm-viewer && trunk build --release --locked'"
  echo "  Trunk.toml requires trunk ${floor:-a pinned version}, and an older trunk refuses the build"
  echo "  the ci.yml viewer job runs this over a real bundle and gates the merge"
  exit 0
fi

breached=0
measured=0

while IFS=$'\t' read -r asset pattern max recorded claim; do
  # One file per asset kind; several would mean the build changed shape.
  found="$(find "$dist" -maxdepth 1 -type f -name "$pattern" | wc -l | tr -d '[:space:]')"
  if [[ "$found" -ne 1 ]]; then
    echo "BREACH $asset: expected one $pattern in $dist, found $found" >&2
    breached=$((breached + 1))
    continue
  fi
  file="$(find "$dist" -maxdepth 1 -type f -name "$pattern")"
  size="$(gzip -9 -c "$file" | wc -c | tr -d '[:space:]')"
  measured=$((measured + 1))
  if [[ "$size" -gt "$max" ]]; then
    breached=$((breached + 1))
    printf 'BREACH %-5s %8d > %8d bytes gzipped  (%s)\n' "$asset" "$size" "$max" "$claim" >&2
  else
    printf 'ok     %-5s %8d <= %8d bytes gzipped  (recorded %d)\n' \
      "$asset" "$size" "$max" "$recorded"
  fi
done < <(jq -r '.bars[] | [.asset, .pattern, .max_gzip_bytes, .measured_gzip_bytes, .claim] | @tsv' "$bars")

echo "bundle-size: $((measured - breached)) of $measured bars hold"
if [[ "$breached" -gt 0 ]]; then
  echo "bundle-size: a breach is bytes to justify or a claim to re-adjudicate, never a bar to raise" >&2
  exit 1
fi

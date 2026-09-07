#!/usr/bin/env bash
# SPDX-License-Identifier: BUSL-1.1
# The viewer bundle every reader downloads, measured gzipped against the two
# numbers in app/ferroterm-viewer/bundle-size.json.
#
#   scripts/checks/bundle-size.sh [--bars FILE] [--dist DIR] [--base REF]
#
# `max_gzip_bytes` is the CEILING: the product claim about what the finished
# viewer costs a reader. It is set from arithmetic over the screens still to
# land, so it does not move screen by screen.
#
# `max_growth_gzip_bytes` is the PER-CHANGE BUDGET, checked with --base: the
# live build is compared to `measured_gzip_bytes` READ FROM THE MERGE BASE, so
# the figure a change is judged against is not in the branch being judged. A
# slice cannot make its own build green by editing a number. Growing the budget
# is a rate change, for every future change at once, and it is read in review.
#
# A breach is bytes to justify or a claim to re-adjudicate, never a bar to
# raise. `docs/viewer.md` section 12 records the basis and the ordered path.
#
# The bundle is built by `trunk build --release` from app/ferroterm-viewer; a
# missing dist/ SKIPS LOUDLY rather than failing, because no ordinary cargo
# gate builds it.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

bars=app/ferroterm-viewer/bundle-size.json
dist=app/ferroterm-viewer/dist
base=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bars) bars=$2; shift 2 ;;
    --dist) dist=$2; shift 2 ;;
    --base) base=$2; shift 2 ;;
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

# The baseline is the bars file as the merge base has it, never as this branch
# has it. Without --base there is nothing to compare against and the growth
# check does not run, which the summary says out loud.
baseline=""
if [[ -n "$base" ]]; then
  if ! baseline="$(git show "$base:$bars" 2>/dev/null)"; then
    echo "bundle-size: cannot read $bars at $base" >&2
    exit 2
  fi
fi

recorded_at_base() {
  if [[ -z "$baseline" ]]; then
    return 1
  fi
  printf '%s' "$baseline" |
    jq -er --arg asset "$1" '.bars[] | select(.asset == $asset) | .measured_gzip_bytes'
}

breached=0
checks=0
grown=0

while IFS=$'\t' read -r asset pattern max growth recorded claim; do
  # One file per asset kind; several would mean the build changed shape.
  found="$(find "$dist" -maxdepth 1 -type f -name "$pattern" | wc -l | tr -d '[:space:]')"
  if [[ "$found" -ne 1 ]]; then
    echo "BREACH $asset: expected one $pattern in $dist, found $found" >&2
    breached=$((breached + 1))
    continue
  fi
  file="$(find "$dist" -maxdepth 1 -type f -name "$pattern")"
  size="$(gzip -9 -c "$file" | wc -c | tr -d '[:space:]')"
  checks=$((checks + 1))
  if [[ "$size" -gt "$max" ]]; then
    breached=$((breached + 1))
    printf 'BREACH %-5s %8d > %8d bytes gzipped, the ceiling  (%s)\n' "$asset" "$size" "$max" "$claim" >&2
  else
    printf 'ok     %-5s %8d <= %8d bytes gzipped  (recorded %d)\n' \
      "$asset" "$size" "$max" "$recorded"
  fi

  if ! was="$(recorded_at_base "$asset")"; then
    continue
  fi
  grown=$((grown + 1))
  checks=$((checks + 1))
  delta=$((size - was))
  if [[ "$delta" -gt "$growth" ]]; then
    breached=$((breached + 1))
    printf 'BREACH %-5s grew %+d bytes over the %d the base recorded, budget %d\n' \
      "$asset" "$delta" "$was" "$growth" >&2
  else
    printf 'ok     %-5s grew %+d bytes, budget %d\n' "$asset" "$delta" "$growth"
  fi
done < <(jq -r '.bars[] | [.asset, .pattern, .max_gzip_bytes, .max_growth_gzip_bytes, .measured_gzip_bytes, .claim] | @tsv' "$bars")

echo "bundle-size: $((checks - breached)) of $checks checks hold"
if [[ "$grown" -eq 0 ]]; then
  echo "bundle-size: no --base given, so only the ceilings were checked"
fi
if [[ "$breached" -gt 0 ]]; then
  {
    echo "bundle-size: a breach is bytes to justify or a claim to re-adjudicate, never a bar to raise"
    echo "  the ordered path is in docs/viewer.md section 12: measure the composition first"
    echo "  a growth breach whose bytes are justified records this build's figure as"
    echo "  measured_gzip_bytes, which moves the baseline for the next change by one change"
  } >&2
  exit 1
fi

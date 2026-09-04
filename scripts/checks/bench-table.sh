#!/usr/bin/env bash
# bench-table.sh: the README's speed and footprint table, rendered from a
# committed set of benchmark records, never typed by hand.
#
#   scripts/checks/bench-table.sh render [DIR]  # print the table for the record set (default: the newest under bench/records)
#   scripts/checks/bench-table.sh check  [DIR]  # fail when README.md's table differs from the rendered one
#
# A record set is one directory under bench/records, named by the date and the
# machine (bench/records/2026-09-04-apple-m2), holding the records one
# ferroterm-bench run wrote (copied there by hand from bench/results). Every
# record in a set must come from the same machine and the same FerroTERM
# version; the newest record per system is rendered. The README carries the
# table between `<!-- bench-table:begin -->` and `<!-- bench-table:end -->`.
set -euo pipefail
cd "$(dirname "$0")/../.."

readonly RECORDS=bench/records
readonly README=README.md

newest_set() {
  local dir
  dir="$(find "$RECORDS" -mindepth 1 -maxdepth 1 -type d | sort | tail -1)"
  if [ -z "$dir" ]; then
    echo "bench-table: no record set under $RECORDS" >&2
    return 1
  fi
  printf '%s\n' "$dir"
}

render() {
  local dir="${1:-$(newest_set)}"
  local files
  files="$(find "$dir" -maxdepth 1 -name '*.json' | sort)"
  if [ -z "$files" ]; then
    echo "bench-table: no records under $dir" >&2
    return 1
  fi
  # shellcheck disable=SC2086 # the file list is newline-separated paths
  jq -r -n -f scripts/checks/bench-table.jq $files
}

check() {
  local rendered current
  rendered="$(render "${1:-}")"
  current="$(awk '/bench-table:begin/{p=1; next} /bench-table:end/{p=0} p' "$README")"
  if [ "$rendered" != "$current" ]; then
    echo "bench-table: README.md's table differs from the records; run: scripts/checks/bench-table.sh render" >&2
    diff <(printf '%s\n' "$current") <(printf '%s\n' "$rendered") >&2 || true
    return 1
  fi
  echo "bench-table: OK (README.md matches the records)."
}

case "${1:-render}" in
  render) render "${2:-}" ;;
  check) check "${2:-}" ;;
  *) echo "usage: $0 render|check [DIR]" >&2; exit 2 ;;
esac

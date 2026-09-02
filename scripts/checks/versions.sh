#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# scripts/checks/versions.sh
#
# Version-drift guard: docs/VERSIONS.md is the single source of truth, and this
# script fails when a config file disagrees with it. It runs on a discovery-phase
# repo, so every check SKIPS LOUDLY (not fails) when the file it compares does not
# exist yet, and activates automatically as the project grows. Run locally or in
# CI (the `versions` job).
#
# Exit 0 = all present checks agree (skips are fine). Exit 1 = a real drift.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

fail=0
note() { printf '  %s\n' "$*"; }
bad() { printf '  DRIFT: %s\n' "$*" >&2; fail=1; }

# --- CITATION.cff version == root Cargo.toml [workspace.package] version -------
echo "== product version (CITATION.cff <-> Cargo.toml)"
cff_ver=""
[ -f CITATION.cff ] && cff_ver="$(grep -m1 '^version:' CITATION.cff | sed -E 's/^version:[[:space:]]*//; s/["'\'']//g' | tr -d '[:space:]' || true)"
if [ -f Cargo.toml ]; then
  # The version line inside the [workspace.package] table.
  cargo_ver="$(awk '/^\[workspace\.package\]/{f=1;next} /^\[/{f=0} f && /^version[[:space:]]*=/{gsub(/[" ]/,""); sub(/^version=/,""); print; exit}' Cargo.toml || true)"
  if [ -z "$cff_ver" ]; then
    bad "Cargo.toml exists but CITATION.cff has no version"
  elif [ -z "$cargo_ver" ]; then
    note "Cargo.toml has no [workspace.package] version yet — skipped"
  elif [ "$cff_ver" != "$cargo_ver" ]; then
    bad "CITATION.cff version ($cff_ver) != Cargo.toml workspace version ($cargo_ver)"
  else
    note "OK: both are $cff_ver"
  fi
else
  note "no Cargo.toml yet — skipped (CITATION.cff version: ${cff_ver:-none})"
fi

# --- rust-toolchain.toml present (sanity; channel recorded in VERSIONS.md) -----
echo "== rust toolchain"
if [ -f rust-toolchain.toml ]; then
  chan="$(grep -m1 -E '^channel[[:space:]]*=' rust-toolchain.toml | sed -E 's/.*=[[:space:]]*//; s/["'\'']//g' | tr -d '[:space:]' || true)"
  note "rust-toolchain.toml channel: ${chan:-unset}"
else
  note "no rust-toolchain.toml yet — skipped (lands with the workspace)"
fi

# --- Vendored FHIR package pins == docs/VERSIONS.md table ----------------------
echo "== vendored FHIR package pins (PROVENANCE.md <-> docs/VERSIONS.md)"
if [ -d tools/notio-fhir-codegen/vendor ]; then
  # TODO: when packages are vendored, parse each PROVENANCE.md version and compare
  # to the docs/VERSIONS.md FHIR table. Not yet vendored.
  note "vendor/ present — per-package PROVENANCE checks activate when packages land"
else
  note "no vendored FHIR packages yet — skipped"
fi

echo
if [ "$fail" -ne 0 ]; then
  echo "versions: DRIFT detected (see above)." >&2
  exit 1
fi
echo "versions: OK (all present checks agree)."

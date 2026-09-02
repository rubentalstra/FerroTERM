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

# --- the server bin is named `ferroterm`, as release-build.yml packages it -------
echo "== server binary name (app/ferroterm-server/Cargo.toml <-> release-build.yml)"
if [ -f app/ferroterm-server/Cargo.toml ]; then
  bin_name="$(awk '/^\[\[bin\]\]/{f=1;next} /^\[/{f=0} f && /^name[[:space:]]*=/{gsub(/[" ]/,""); sub(/^name=/,""); print; exit}' app/ferroterm-server/Cargo.toml || true)"
  if [ "$bin_name" != "ferroterm" ]; then
    bad "app/ferroterm-server/Cargo.toml [[bin]] name is '${bin_name:-unset}', release-build.yml packages 'ferroterm'"
  else
    note "OK: the bin is ferroterm"
  fi
else
  note "no app/ferroterm-server/Cargo.toml yet — skipped"
fi

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
if [ -d tools/ferroterm-fhir-codegen/vendor ]; then
  found=0
  for prov in tools/ferroterm-fhir-codegen/vendor/*/PROVENANCE.md; do
    [ -f "$prov" ] || continue
    found=1
    pkg="$(basename "$(dirname "$prov")")"
    prov_ver="$(sed -nE 's/^- Version:[[:space:]]*//p' "$prov" | head -n1 | tr -d '[:space:]')"
    # The second cell of the package's row in the docs/VERSIONS.md FHIR table.
    pin_ver="$(awk -F'|' -v pkg="$pkg" '$2 ~ "^[[:space:]]*`" pkg "`" { v = $3; gsub(/^[[:space:]]+|[[:space:]]+$/, "", v); print v; exit }' docs/VERSIONS.md)"
    pkg_json="tools/ferroterm-fhir-codegen/vendor/$pkg/package/package.json"
    json_ver=""
    [ -f "$pkg_json" ] && json_ver="$(sed -nE 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' "$pkg_json" | head -n1)"
    if [ -z "$prov_ver" ]; then
      bad "$prov has no '- Version:' line"
    elif [ -z "$pin_ver" ]; then
      bad "$pkg is vendored ($prov_ver) but has no row in the docs/VERSIONS.md FHIR table"
    elif [ "$prov_ver" != "$pin_ver" ]; then
      bad "$pkg: PROVENANCE.md says $prov_ver, docs/VERSIONS.md pins $pin_ver"
    elif [ -n "$json_ver" ] && [ "$json_ver" != "$prov_ver" ]; then
      bad "$pkg: package.json says $json_ver, PROVENANCE.md says $prov_ver"
    else
      note "OK: $pkg $prov_ver (PROVENANCE.md, package.json, and the pin table agree)"
    fi
  done
  [ "$found" -eq 1 ] || note "vendor/ present but holds no PROVENANCE.md yet — skipped"
else
  note "no vendored FHIR packages yet — skipped"
fi

echo
if [ "$fail" -ne 0 ]; then
  echo "versions: DRIFT detected (see above)." >&2
  exit 1
fi
echo "versions: OK (all present checks agree)."

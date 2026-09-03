#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# scripts/vendor/fhir-packages.sh
#
# Vendors the pinned HL7 FHIR packages the generator consumes
# (.claude/rules/vendored-inputs.md). For each package it reads the pin from
# the FHIR table in docs/VERSIONS.md (the single source of truth), downloads
# that exact version from the FHIR package registry, verifies the tarball
# against the registry's recorded checksum, extracts it verbatim into
# tools/ferroterm-fhir-codegen/vendor/<package>/package/, and writes a
# PROVENANCE.md beside it. Re-running with unchanged pins reproduces the same
# tree byte for byte (only the fetch date in PROVENANCE.md moves).
#
# Usage:
#   scripts/vendor/fhir-packages.sh                # the default set below
#   scripts/vendor/fhir-packages.sh hl7.terminology  # one or more packages
#
# Requires: curl, jq, tar, shasum.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

registry="https://packages.fhir.org"
registry2="https://packages2.fhir.org/packages"
vendor_dir="tools/ferroterm-fhir-codegen/vendor"
pins="docs/VERSIONS.md"
default_packages=(hl7.fhir.r4.core hl7.fhir.r4b.core hl7.fhir.r5.core hl7.fhir.r6.core hl7.terminology)

die() { printf 'fhir-packages: %s\n' "$*" >&2; exit 1; }

for tool in curl jq tar shasum; do
  command -v "$tool" >/dev/null 2>&1 || die "missing required tool: $tool"
done

# The pin for a package: the second cell of its row in the docs/VERSIONS.md
# FHIR table, e.g. "| `hl7.terminology` (THO) | 7.3.0 | ... |".
pin_for() {
  local pkg="$1"
  awk -F'|' -v pkg="$pkg" '
    $2 ~ "^[[:space:]]*`" pkg "`" {
      v = $3; gsub(/^[[:space:]]+|[[:space:]]+$/, "", v); print v; exit
    }' "$pins"
}

vendor_one() {
  local pkg="$1" ver dest meta shasum tarball_url tmp got sha256 license fetched
  ver="$(pin_for "$pkg")"
  [ -n "$ver" ] || die "no pin for $pkg in $pins (add a row to the FHIR table)"
  case "$ver" in
    *[!A-Za-z0-9.-]*) die "pin for $pkg is not a plain version: '$ver'" ;;
  esac
  dest="$vendor_dir/$pkg"

  echo "== $pkg $ver"
  # The R6 ballots are published on packages2.fhir.org only; every other
  # package comes from packages.fhir.org. Both speak the npm registry shape.
  local meta_url
  case "$pkg" in
    hl7.fhir.r6.*) meta_url="$registry2/$pkg" ;;
    *) meta_url="$registry/$pkg" ;;
  esac
  meta="$(curl -fsSL -A "ferroterm-vendor (scripts/vendor/fhir-packages.sh)" "$meta_url")" \
    || die "cannot read registry metadata for $pkg"
  shasum="$(printf '%s' "$meta" | jq -r --arg v "$ver" '.versions[$v].dist.shasum // empty')"
  tarball_url="$(printf '%s' "$meta" | jq -r --arg v "$ver" '.versions[$v].dist.tarball // empty')"
  [ -n "$shasum" ] || die "the registry lists no version $ver of $pkg"

  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  [ -n "$tarball_url" ] || die "the registry lists no tarball for $pkg $ver"
  curl -fsSL -A "ferroterm-vendor (scripts/vendor/fhir-packages.sh)" -o "$tmp/pkg.tgz" "$tarball_url" \
    || die "download of $pkg $ver failed"
  got="$(shasum -a 1 "$tmp/pkg.tgz" | cut -d' ' -f1)"
  [ "$got" = "$shasum" ] || die "checksum mismatch for $pkg $ver: registry $shasum, downloaded $got"
  sha256="$(shasum -a 256 "$tmp/pkg.tgz" | cut -d' ' -f1)"

  mkdir -p "$tmp/x"
  tar -xzf "$tmp/pkg.tgz" -C "$tmp/x"
  [ -f "$tmp/x/package/package.json" ] || die "$pkg $ver has no package/package.json"
  # A FHIR package never carries SNOMED CT release files; refuse anything that
  # looks like RF2 before it can reach the tree (.claude/rules/vendored-inputs.md).
  if find "$tmp/x" -type f \( -name 'sct2_*' -o -name 'der2_*' -o -name '*.rf2' \) | grep -q .; then
    die "$pkg $ver contains RF2-shaped files; SNOMED CT content is never vendored"
  fi
  license="$(jq -r '.license // "unstated"' "$tmp/x/package/package.json")"
  fetched="$(date -u +%Y-%m-%d)"

  rm -rf "$dest"
  mkdir -p "$dest"
  mv "$tmp/x/package" "$dest/package"
  cat > "$dest/PROVENANCE.md" <<PROV
# Provenance: $pkg

Vendored verbatim as codegen input (.claude/rules/vendored-inputs.md). Never
edit a file under \`package/\`; change the pin in docs/VERSIONS.md and re-run
\`scripts/vendor/fhir-packages.sh $pkg\`.

- Package: $pkg
- Version: $ver
- Source: the FHIR package registry, $meta_url
- Tarball: $tarball_url
- SHA-1 (registry shasum): $shasum
- SHA-256 (tarball): $sha256
- Fetched: $fetched
- Upstream license: $license (the \`license\` field of \`package/package.json\`)
- Layout: the tarball's \`package/\` directory, extracted unchanged
PROV
  echo "   $(find "$dest/package" -type f | wc -l | tr -d ' ') files, license $license, sha256 $sha256"
}

packages=("$@")
[ "${#packages[@]}" -gt 0 ] || packages=("${default_packages[@]}")
for pkg in "${packages[@]}"; do
  vendor_one "$pkg"
done
echo "fhir-packages: done"

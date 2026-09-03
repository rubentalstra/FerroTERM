#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The crate-line bump rule (.claude/rules/crates-publishing.md): a change that
# alters the PACKAGED content of any `crates/*` member (what its `include`
# ships: `src/**`, `data/**`, `README.md`, `LICENSE`, `Cargo.toml`) bumps the
# lockstep crate version in the same change, because a published version is
# immutable. A root `[workspace.dependencies]` entry a member consumes is
# packaged content too: `cargo package` renders the concrete requirement.
#
#   crate-version-guard.sh <base-ref> [head-ref]
#
# Exit 0 when no packaged content changed, or it changed and the version moved
# (in every member and every internal requirement, and Cargo.lock followed).
# Exit 1 on a packaged change without a bump, a split bump, or a stale lock.
# The `no-crate-bump` PR label is the CI escape for a diff that provably does
# not alter packaged bytes; this script does not read labels.
set -euo pipefail
cd "$(dirname "$0")/../.."

base="${1:?usage: crate-version-guard.sh <base-ref> [head-ref]}"
head="${2:-HEAD}"

changed="$(git diff --name-only "$base" "$head" --)"
packaged=0
if printf '%s\n' "$changed" | grep -qE '^crates/[a-z0-9-]+/(src/|data/|README\.md$|LICENSE$|Cargo\.toml$)'; then
  packaged=1
fi
if [ "$packaged" -eq 0 ] && printf '%s\n' "$changed" | grep -qx 'Cargo.toml'; then
  diff_names="$(git diff "$base" "$head" -- Cargo.toml | grep -E '^[+-][A-Za-z0-9_-]+[[:space:]]*=' | sed -E 's/^[+-]//; s/[[:space:]]*=.*//' | sort -u || true)"
  for name in $diff_names; do
    if grep -lE "^${name}(\.workspace)?[[:space:]]*=" crates/*/Cargo.toml >/dev/null 2>&1; then
      echo "crate-version-guard: workspace dependency '$name' changed and crates/* members consume it: packaged requirements move."
      packaged=1
      break
    fi
  done
fi

if [ "$packaged" -eq 0 ]; then
  echo "crate-version-guard: no packaged content of crates/* changed."
  exit 0
fi

package_version() {
  awk -F'"' '/^\[package\]/{p=1} p && /^version = /{print $2; exit}'
}
old_ver="$(git show "$base:crates/fhir-types/Cargo.toml" 2>/dev/null | package_version || true)"
new_ver="$(package_version < crates/fhir-types/Cargo.toml)"
if [ -n "$old_ver" ] && [ "$old_ver" = "$new_ver" ]; then
  echo "::error::packaged content of crates/* changed but the crate version is still $new_ver. Bump every crates/*/Cargo.toml 'version' and every internal 'version =' requirement in the root Cargo.toml to the next 0.x, refresh Cargo.lock, or apply the 'no-crate-bump' label when the diff provably does not alter packaged bytes." >&2
  exit 1
fi

fail=0
for manifest in crates/*/Cargo.toml; do
  ver="$(package_version < "$manifest")"
  [ "$ver" = "$new_ver" ] || { echo "::error::$manifest is at $ver, the crate line is $new_ver (bumps are lockstep)." >&2; fail=1; }
  name="$(awk -F'"' '/^\[package\]/{p=1} p && /^name = /{print $2; exit}' "$manifest")"
  req="$(grep -E "^${name} = \{ path = \"crates/${name}\", version = \"" Cargo.toml | sed -E 's/.*version = "([^"]+)".*/\1/' || true)"
  [ "$req" = "$new_ver" ] || { echo "::error::root Cargo.toml requires $name $req, the crate line is $new_ver." >&2; fail=1; }
  locked="$(awk -v n="\"$name\"" '$1 == "name" && $3 == n { hit = 1; next } hit && $1 == "version" { gsub(/"/, "", $3); print $3; exit }' Cargo.lock)"
  [ "$locked" = "$new_ver" ] || { echo "::error::Cargo.lock records $name ${locked:-nothing}, the crate line is $new_ver; run cargo update -w and commit the lock." >&2; fail=1; }
done
[ "$fail" -eq 0 ] || exit 1
echo "crate-version-guard: packaged content changed and the crate line moved ${old_ver:-<new>} -> $new_ver, lockstep and locked."

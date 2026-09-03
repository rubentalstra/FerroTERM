#!/usr/bin/env bash
# Fetches the public registries the registry code systems are built from,
# verbatim, into crates/ferroterm-terminology/data/, and rewrites the
# provenance stamp. Re-run to refresh; commit the result.
#
#   scripts/vendor/registries.sh
#
# Sources (all freely redistributable):
#   IANA Language Subtag Registry (BCP 47) and the IANA Media Types registry
#   (BCP 13): IANA dedicates the protocol registries to the public domain
#   (https://www.iana.org/help/licensing-terms).
#   Unicode CLDR (ISO 3166-1 codes and English territory names): the Unicode
#   License v3 (https://github.com/unicode-org/cldr-json/blob/main/LICENSE).
set -euo pipefail

cd "$(dirname "$0")/../.."
data=crates/ferroterm-terminology/data
mkdir -p "$data/iana/media-types" "$data/cldr"

CLDR_REF=${CLDR_REF:-main}

curl -sSL -o "$data/iana/language-subtag-registry" \
  https://www.iana.org/assignments/language-subtag-registry/language-subtag-registry
for top in application audio font haptics image message model multipart text video; do
  curl -sSL -o "$data/iana/media-types/$top.csv" "https://www.iana.org/assignments/media-types/$top.csv"
done
curl -sSL -o "$data/cldr/territories.json" \
  "https://raw.githubusercontent.com/unicode-org/cldr-json/$CLDR_REF/cldr-json/cldr-localenames-full/main/en/territories.json"
curl -sSL -o "$data/cldr/codeMappings.json" \
  "https://raw.githubusercontent.com/unicode-org/cldr-json/$CLDR_REF/cldr-json/cldr-core/supplemental/codeMappings.json"
curl -sSL -o "$data/cldr/LICENSE" "https://raw.githubusercontent.com/unicode-org/cldr-json/$CLDR_REF/LICENSE"

registry_date=$(sed -n 's/^File-Date: //p' "$data/iana/language-subtag-registry" | head -n1)
cldr_version=$(jq -r '.supplemental.version._cldrVersion' "$data/cldr/codeMappings.json")
cldr_commit=$(curl -sSL "https://api.github.com/repos/unicode-org/cldr-json/commits/$CLDR_REF" | jq -r '.sha')
today=$(date -u +%Y-%m-%d)

cat > "$data/PROVENANCE.md" <<PROV
# Provenance: registry data

Vendored verbatim as the input of the registry code systems (BCP 47, BCP 13,
ISO 3166); never edit a file here, re-run \`scripts/vendor/registries.sh\`.

## IANA Language Subtag Registry (BCP 47)

- File: \`iana/language-subtag-registry\` (record-jar, RFC 5646 §3.1)
- Source: <https://www.iana.org/assignments/language-subtag-registry/language-subtag-registry>
- Registry File-Date: $registry_date
- Fetched: $today
- License: public domain (IANA protocol registries, <https://www.iana.org/help/licensing-terms>)

## IANA Media Types registry (BCP 13)

- Files: \`iana/media-types/<type>.csv\` for the ten top-level types
- Source: <https://www.iana.org/assignments/media-types/media-types.xhtml> (the per-type CSV exports)
- Fetched: $today
- License: public domain (IANA protocol registries, <https://www.iana.org/help/licensing-terms>)

## Unicode CLDR (ISO 3166-1)

- Files: \`cldr/codeMappings.json\` (alpha-2 to alpha-3 and numeric), \`cldr/territories.json\` (English names), \`cldr/LICENSE\`
- Source: <https://github.com/unicode-org/cldr-json>, ref \`$CLDR_REF\` at commit \`$cldr_commit\`
- CLDR version: $cldr_version
- Fetched: $today
- License: Unicode License v3 (\`cldr/LICENSE\`)
PROV
echo "fetched: registry $registry_date, CLDR $cldr_version ($cldr_commit)"

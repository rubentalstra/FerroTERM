#!/usr/bin/env bash
# The HL7 terminology ecosystem suite against a FerroTERM, with a committed
# pass list: a test on the list that fails is a regression, a test that
# newly passes is reported so it can be added.
#
#   scripts/checks/tx-ecosystem.sh [--server URL] [--out DIR]
#
# Without --server the script starts target/release/ferroterm on 127.0.0.1:8098
# (build it first: cargo build --release -p ferroterm-server). The validator
# jar and the suite are fetched into target/tx-ecosystem/ once and pinned by
# digest and commit. A JVM runs here only, never in the server.
set -euo pipefail

VALIDATOR_VERSION=6.10.3
VALIDATOR_SHA256=91e4da9d1bd4c11d9a05c0ec0837c0c830ef800bc37faed6873e26f6702bceba
SUITE_REPO=https://github.com/HL7/fhir-tx-ecosystem-ig
SUITE_COMMIT=eaec771d82fba4eac596c14963546f39b4ecffe7
PASSING=conformance/tx-ecosystem/passing.txt

server=""
out=target/tx-ecosystem/out
while [ $# -gt 0 ]; do
  case "$1" in
    --server) server=$2; shift 2 ;;
    --out) out=$2; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

work=target/tx-ecosystem
mkdir -p "$work"
jar=$work/validator_cli-$VALIDATOR_VERSION.jar
if [ ! -f "$jar" ]; then
  echo "fetching validator_cli.jar $VALIDATOR_VERSION"
  curl -sSL -o "$jar.tmp" "https://github.com/hapifhir/org.hl7.fhir.core/releases/download/$VALIDATOR_VERSION/validator_cli.jar"
  mv "$jar.tmp" "$jar"
fi
echo "$VALIDATOR_SHA256  $jar" | shasum -a 256 -c - >/dev/null

suite=$work/suite
if [ ! -d "$suite/tests" ]; then
  echo "fetching the suite at $SUITE_COMMIT"
  rm -rf "$suite"
  git init -q "$suite"
  git -C "$suite" remote add origin "$SUITE_REPO"
  git -C "$suite" sparse-checkout set tests
  git -C "$suite" fetch -q --depth 1 origin "$SUITE_COMMIT"
  git -C "$suite" checkout -q FETCH_HEAD
fi

started=""
if [ -z "$server" ]; then
  server=http://127.0.0.1:8098/r4b
  FERROTERM_LISTEN=127.0.0.1:8098 FERROTERM_LOG_FORMAT=json target/release/ferroterm > "$work/server.log" 2>&1 &
  started=$!
  trap 'kill "$started" 2>/dev/null || true' EXIT
  for _ in $(seq 1 50); do
    curl -sf "http://127.0.0.1:8098/health" >/dev/null 2>&1 && break
    sleep 0.2
  done
fi

rm -rf "$out"
mkdir -p "$out"
# The runner exits non-zero while any test fails; the pass list decides here.
java -jar "$jar" txTests -tx "$server" -test-version "$suite/tests" -mode general \
  -output "$out" -ssrf-protection-enabled=false > "$out/runner.log" 2>&1 || true
report=$out/report.json
if [ ! -f "$report" ]; then
  echo "the runner produced no report; see $out/runner.log" >&2
  tail -n 40 "$out/runner.log" >&2
  exit 1
fi

jq -r '.test[] | select(.action[0].operation.result == "pass") | .name' "$report" | sort > "$out/passing.txt"
total=$(jq '.test | length' "$report")
passed=$(wc -l < "$out/passing.txt" | tr -d ' ')
echo "tx-ecosystem: $passed of $total general tests pass ($(grep -a -o 'tests v[0-9.]*' "$out/runner.log" | head -1), runner $VALIDATOR_VERSION)"

regressions=$(comm -23 "$PASSING" "$out/passing.txt")
gains=$(comm -13 "$PASSING" "$out/passing.txt")
if [ -n "$gains" ]; then
  echo "newly passing (add them to $PASSING):"
  for name in $gains; do echo "  + $name"; done
fi
if [ -n "$regressions" ]; then
  echo "REGRESSION: on the pass list but failing now:" >&2
  for name in $regressions; do
    echo "  - $name" >&2
    jq -r --arg n "$name" '.test[] | select(.name == $n) | "    \(.action[0].operation.message // "-" | .[0:300])"' "$report" >&2
  done
  exit 1
fi
echo "tx-ecosystem: no regression against $PASSING"

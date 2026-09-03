#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# scripts/gh/labels.sh — bootstrap the FerroTERM issue-label taxonomy.
#
# Creates (idempotently) the labels the tracker workflow assumes:
#   * exactly one TYPE label per issue, mapped to the conventional-commit types
#     (bug/enhancement/documentation/chore/refactor/perf/ci);
#   * a PRIORITY (P0-P3);
#   * a few DOMAIN/area labels.
#
# `gh label create --force` updates an existing label in place, so re-running
# this is safe and converges the colours/descriptions to the values below.
# Run it ONCE after the repository is created (owner action).
#
# Taxonomy policy: .claude/rules/issue-workflow.md
# Official docs: https://cli.github.com/manual/gh_label_create
#
# Usage: scripts/gh/labels.sh

set -euo pipefail

die() {
  echo "gh-labels: $*" >&2
  exit 1
}

command -v gh >/dev/null 2>&1 || die "the GitHub CLI (gh) is not installed"
gh repo view --json nameWithOwner --jq .nameWithOwner >/dev/null 2>&1 ||
  die "could not resolve the current repository (run inside a gh-authenticated clone)"

# label <name> <hex-color> <description>
label() {
  gh label create "$1" --color "$2" --description "$3" --force >/dev/null
  echo "ok: $1"
}

echo "== type labels (exactly one per issue; maps to the conventional-commit type) =="
label bug           d73a4a "A defect. Maps to a fix/ branch and a fix: commit."
label enhancement   a2eeef "A new capability. Maps to a feat/ branch and a feat: commit."
label documentation 0075ca "Docs-only work. Maps to a docs/ branch and a docs: commit."
label chore         fef2c0 "Maintenance with no product change. chore/ and chore:."
label refactor      cfd3d7 "Behaviour-preserving code change. refactor/ and refactor:."
label perf          fbca04 "Performance work. perf/ and perf:."
label test          c2e0c6 "Test-only work. test/ and test:."
label ci            ededed "CI, build, and tooling. ci/ and ci:."

echo "== priority labels (P0 critical .. P3 backlog) =="
label P0 b60205 "Critical, drop everything."
label P1 d93f0b "High, current focus."
label P2 fbca04 "Normal."
label P3 0e8a16 "Backlog."

echo "== domain / area labels =="
label spec:FHIR    5319e7 "FHIR terminology wire (operations, versioning, OperationOutcome)."
label spec:SNOMED  1d76db "SNOMED CT and RF2 semantics."
label spec:ECL     006b75 "Expression Constraint Language: parser and evaluator."
label codegen      c5def5 "The ferroterm-fhir generator (tools/ferroterm-fhir-codegen)."
label server       bfd4f2 "The axum HTTP server (app/ferroterm-server)."
label storage      d4c5f9 "The store, graph, text index, and redb persistence."
label infra        bfdadc "CI/CD, supply chain, and deployment."
label website      f9d0c4 "The docs site (website/book) and landing page."

echo "== workflow / meta labels =="
label dependencies 0366d6 "Dependency updates (used by Dependabot)."
label security     ee0701 "Security fix or hardening."
label blocked      000000 "Blocked by another open issue (see the dependencies panel)."
label blocked-upstream 6f42c1 "Waiting on an upstream spec or tool release."
label upstream-report  990000 "An outbound report of a defect in the FHIR or SNOMED specs."

echo "done."

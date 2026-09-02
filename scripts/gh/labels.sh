#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# scripts/gh/labels.sh — bootstrap the Notio issue-label taxonomy.
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
label bug           d73a4a "A defect — maps to a fix/ branch + fix: commit"
label enhancement   a2eeef "A new capability — maps to a feat/ branch + feat: commit"
label documentation 0075ca "Docs-only work — maps to a docs/ branch + docs: commit"
label chore         fef2c0 "Maintenance with no product change — chore/ + chore:"
label refactor      cfd3d7 "Behaviour-preserving code change — refactor/ + refactor:"
label perf          fbca04 "Performance work — perf/ + perf:"
label ci            ededed "CI / build tooling — ci/ + ci:"

echo "== priority labels (P0 critical .. P3 backlog) =="
label P0 b60205 "Critical — drop everything"
label P1 d93f0b "High — current focus"
label P2 fbca04 "Normal"
label P3 0e8a16 "Backlog"

echo "== domain / area labels =="
label spec:FHIR    5319e7 "FHIR terminology wire (operations, versioning, OperationOutcome)"
label spec:SNOMED  1d76db "SNOMED CT / ECL / RF2 semantics"
label codegen      c5def5 "The notio-fhir generator (tools/notio-fhir-codegen)"
label server       bfd4f2 "The axum HTTP server (app/notio-server)"

echo "done."

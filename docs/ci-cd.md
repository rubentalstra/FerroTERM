# CI/CD and supply-chain

No spec governs this: our own design, grounded in SLSA v1.0, OpenSSF, Sigstore,
and the OWASP GitHub Actions Security Cheat Sheet. The enforceable discipline is
`.claude/rules/ci-cd.md`; this document is the design and the rationale.

The foundation is stood up before product code so it is not an afterthought. It
is built to stay green on a pre-code repo: config/workflow/shell analysis runs
now; Rust and release lanes activate automatically when the workspace and tags
exist.

## The workflows

| Workflow | Does | Active |
|---|---|---|
| `ci.yml` (workflows lane) | `actionlint`, `zizmor --min-severity=low` over `.github/workflows/`, `shellcheck` over tracked shell | now |
| `ci.yml` (Rust lanes) | fmt, clippy `-D warnings`, nextest `--locked` + doctests, rustdoc `-D warnings`, `cargo deny`, MSRV (`cargo hack check --rust-version`), dependency-review (PR) | on workspace |
| `ci.yml` (`versions` job) | `scripts/checks/versions.sh`, version-drift guard | now |
| `scorecard.yml` | OpenSSF Scorecard, results published, SARIF → code scanning | now |
| `codeql.yml` | CodeQL: `actions` now, `rust` self-activates when the workspace lands | actions now |
| `sonar.yml` | SonarQube Cloud (advisory; `.claude/rules/ai-code-review.md`) | now (sweep); coverage on workspace |
| `release-build.yml` | reusable SLSA Build L3 lane: build + SBOM + keyless attest | on tag |
| `release.yml` | `v*` orchestrator: draft → per-arch build → verify assets → publish last | on tag |

Dependabot (`github-actions` + `cargo`) keeps `uses:` SHAs and crate pins current.

## SLSA Build Level 3

L3 adds two guarantees over L2: builds are isolated from each other, and the
signing identity is not reachable by user build steps ("provenance is
unforgeable"). On GitHub-hosted runners this is achieved by putting the build
**and** its attestation in a **reusable workflow** (`release-build.yml`,
`on: workflow_call`). It runs on its own VM the caller cannot inject steps into,
so the Sigstore/Fulcio identity that signs names *that workflow*. Do not inline
the build+attest into a normal job; that is only L2.

Consumers verify provenance against the signer workflow:

```
gh attestation verify ferroterm-<tag>-<target>.tar.gz -R rubentalstra/ferroterm \
  --signer-workflow rubentalstra/ferroterm/.github/workflows/release-build.yml
```

We claim L3 by GitHub's builder isolation; we do not (yet) claim reproducible or
hermetic builds; those are separate SLSA tracks.

## SBOM and signing

Each release artifact carries: an embedded dependency list (`cargo auditable`
writes it into the binary's `.dep-v0` section), a CycloneDX SBOM
(`cargo cyclonedx`), and a `.sha256`. The provenance and the SBOM are signed
**keyless** via Sigstore (`actions/attest-build-provenance` and
`actions/attest-sbom`, `id-token: write`), so there is no long-lived key. A
"signed SBOM" here is the SBOM wrapped in a Sigstore DSSE bundle bound to the
artifact digest and signed by the pinnable workflow identity, verifiable with
`gh attestation verify`. OCI images (later) get BuildKit `provenance: mode=max` +
`sbom: true`, a registry-pushed attestation, and a keyless `cosign sign` by
digest.

## OpenSSF

- **Scorecard** runs weekly and on push/branch-protection changes, publishing
  results. We earn the checks honestly: pinned dependencies (SHA-pinned `uses:` +
  Dependabot digest bumps), minimal token permissions, no dangerous-workflow
  patterns, branch protection, CodeQL (SAST + code scanning), Dependabot
  (dependency-update-tool), signed releases, a security policy. Two checks score
  low for a new solo repo by construction: **Maintained** (needs 90-day
  activity) and **Code-Review** (needs a second human); these are accepted, not
  chased.
- **Best Practices badge:** register at bestpractices.dev and fill the passing
  self-assessment (version control, unique versioning, issue tracker, license,
  basic docs, HTTPS, no known-unpatched vulns are already satisfiable). Silver
  and gold follow as the project matures; SLSA provenance + signed releases
  materially help the higher tiers.

## Rust supply chain

`cargo deny check` (advisories/licenses/bans/sources, subsumes cargo-audit),
`Cargo.lock` committed with every CI build `--locked`, MSRV verified with
`cargo hack check --rust-version`, and `dependency-review-action` blocking
vulnerable or license-incompatible new deps on PRs. `cargo-vet` and a `cargo-fuzz`
harness are maturity additions.

## GitHub hardening

Every `uses:` SHA-pinned; `permissions: {}` + least-privilege per job;
`persist-credentials: false`; no context interpolation in `run:`; no build cache
in publishing lanes. A `main` ruleset requires PRs, status checks, signed
commits, and blocks force-push and deletion. Releases are immutable (freeze on
publish; recover with a new patch version, never a retag).

## Owner actions (one-time, cannot be scripted)

- `main` ruleset: require PR + status checks, **require signed commits**, block
  force-push + deletion.
- Enable: code scanning, secret scanning + push protection, Dependabot
  alerts/updates, artifact attestations. (Immutable releases: already enabled.)
- Add the `SONAR_TOKEN` repository secret. (Done.) Keep SonarCloud **Automatic
  Analysis OFF** (done) so CI-based analysis is authoritative.
- Register the project at bestpractices.dev; add the returned badge to the
  README.
- Run `scripts/gh/labels.sh` once to create the label taxonomy; create the
  "FerroTERM Roadmap" Project if the board is wanted.

## Sources

SLSA v1.0 <https://slsa.dev/spec/v1.0/levels> · GitHub artifact attestations
<https://docs.github.com/actions/security-guides/using-artifact-attestations-and-reusable-workflows-to-achieve-slsa-v1-build-level-3>
· SBOM CycloneDX <https://cyclonedx.org/> · `cargo-auditable`
<https://github.com/rust-secure-code/cargo-auditable> · OpenSSF Scorecard
<https://securityscorecards.dev/> · Best Practices <https://www.bestpractices.dev/>
· Sigstore cosign <https://docs.sigstore.dev/cosign/> · cargo-deny
<https://embarkstudios.github.io/cargo-deny/> · OWASP GHA Cheat Sheet
<https://cheatsheetseries.owasp.org/cheatsheets/GitHub_Actions_Security_Cheat_Sheet.html>
· zizmor <https://docs.zizmor.sh/>

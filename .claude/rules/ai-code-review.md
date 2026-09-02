# Machine review (SonarQube Cloud): advisory, never authority

Every pull request and every push to `main` is analyzed by SonarQube Cloud
(`.github/workflows/sonar.yml`; scope in `sonar-project.properties`; project
`rubentalstra_ferroterm`, org `rubentalstra`). Rust is analyzed first-party (the
analyzer runs Clippy over the workspace); shell, YAML, and other languages are
covered by the multi-language sweep. It exists as a second opinion beside the
local gates and CodeQL.

It is a **second opinion**. It is not authority, and it gates no merge.

## Precedence: a finding never outranks the sources

1. The FHIR + SNOMED/ECL specifications (`spec-adherence.md`): the oracle.
2. The hard rules: `CLAUDE.md`, the `.claude/rules/*.md`.
3. The local gates: `cargo fmt`, `clippy`, `cargo nextest`, `cargo deny`, the CI
   guards.
4. The analyzer.

A finding that contradicts a spec citation, or asks for something the rules
forbid, is wrong by construction. Nothing it reports relaxes `testing.md`: never
weaken a test because a finding suggested it.

## Rules

- **It gates no merge.** The quality gate is informational; the README badge
  reflects it, but merges are decided by the local gates + review.
- **Findings are acted on by hand, in a normal change**, never applied through
  a UI that would attribute a commit to a bot (the no-AI-attribution rule has no
  exceptions).
- **Automatic Analysis stays OFF** (owner setting): CI-based analysis and
  SonarCloud Automatic Analysis cannot both run. `sonar.yml` is the analysis
  path.
- **A wrong finding is data, not a silent suppression:** record it (a tracker
  issue or a scope adjustment in `sonar-project.properties`), and only change
  scope when the scope is genuinely wrong.

CodeQL (security) runs separately (`.github/workflows/codeql.yml`) and is also
advisory here until a precision case is made to gate on it.

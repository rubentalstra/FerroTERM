# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Maintenance rule: every pull request that changes user-visible behaviour (the
REST/terminology surface, ECL, validation, configuration, CLI, or
container/deployment artifacts) adds an entry under **[Unreleased]** in the
same PR. Cutting a release renames [Unreleased] to the version + date and adds a
fresh link reference.

## [Unreleased]

### Changed

- The engine is code-system-neutral by design: the FHIR terminology operations
  talk to a code system provider seam, SNOMED CT is the first provider, and
  LOINC, UCUM, ICD-10, and the other systems in `docs/terminologies.md` follow
  through the same seam.

### Added

- The Cargo workspace: the seven engine crates, the `notio-server` binary
  (with a `GET /health` route), and the two tools, with the pinned dependency
  set, the workspace lint table, and the Rust CI lanes active.
- Project foundation: architecture, `.claude/` project configuration (rules,
  agents, hooks, skills, memory), CI/CD + supply-chain scaffolding, the tracker
  work-style, and citation/funding metadata.

# Configuration

This page describes how you configure the server. The configuration surface is in
design, so the setting names below are the planned shape and may change before the
first release.

> [!NOTE]
> No configuration schema is frozen yet. This page lists the settings the design
> calls for. Check the release notes for the exact names when a build ships.

<!-- toc -->

## What you configure

A deployment configures a small set of things:

- **The index path.** Where the built SNOMED CT index lives. The server opens it
  read-only at startup.
- **The listen address and port.** Where the server accepts FHIR requests.
- **The served FHIR versions.** Which of R4, R4B, R5, and R6 this deployment
  answers. See [FHIR versions](../integrate/fhir-versions.md).
- **Expansion limits.** The default and maximum page size for `ValueSet/$expand`,
  so a broad expansion cannot return an unbounded result in one response. See
  [Implicit value sets and ECL](../integrate/ecl-value-sets.md).
- **Logging.** The log level and format for the `tracing` output.

## How you set it

The design follows the common Rust server pattern: command-line flags for the
essentials, environment variables for a container, and a file for the rest, with
flags taking precedence over the environment and the environment over the file.
The exact grammar is settled with the first build.

## What you do not configure

You do not configure a database connection, a search cluster, or a JVM heap.
FerroTERM has none of these. The one input the server needs is the built index, and
you build that offline with the release-loading tool described in
[Loading a SNOMED CT edition](loading-snomed.md).

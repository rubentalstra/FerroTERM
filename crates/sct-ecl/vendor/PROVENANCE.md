# Provenance: the SNOMED CT Expression Constraint Language grammar

- Source: <https://github.com/IHTSDO/snomed-expression-constraint-language>
- Tag: 2.2
- Commit: e0b83c15441932e5953e4b3bfee48e0a249ab403
- Fetched: 2026-09-03 by `scripts/vendor/ecl-grammar.sh`
- Licence: Apache License 2.0 (`LICENSE.md`, vendored verbatim)
- Contents: `syntax/` (the ANTLR grammar `ECL.g4` and the ABNF forms) and
  `examples/` (the valid example corpus), copied verbatim; `README.md`.

The parser in `crates/ferroterm-ecl` mirrors `syntax/ECL.g4` rule for
rule; the corpus is the parse-conformance fixture. Never hand-edit these
files; change the pin in `docs/VERSIONS.md` and re-run the script.

# Provenance: the SNOMED CT Compositional Grammar syntax

- Source: <https://github.com/IHTSDO/SNOMEDCT-Languages>
- Commit: 23d3812cf17ac8459fc9f1a0068041b95bacd3e1
- Fetched: 2026-09-23 by `scripts/vendor/scg-grammar.sh`
- Licence: Apache License 2.0 (`LICENSE.md`, vendored verbatim)
- Contents: `syntax/` (the normative ABNF) and `examples/` (the valid
  example corpus), copied verbatim from `SnomedCTCompositionalGrammar/`;
  `README.md`. Each example keeps its content and is renamed from
  `CGv2 example (<name>).txt` to `<name>.txt` so a glob and a test can
  carry it.

The parser in `crates/sct-scg` mirrors `syntax/` rule for rule; the corpus
is the parse-conformance fixture. Never hand-edit these files; change the pin
in `docs/VERSIONS.md` and re-run the script.

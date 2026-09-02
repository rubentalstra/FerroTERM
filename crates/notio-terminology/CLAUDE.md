# notio-terminology

The engine. Hand-written; the FHIR terminology module for the served version
and the SNOMED CT URI standard are the authority
(`.claude/rules/fhir-terminology.md`, `snomed-terminology.md`).

- Consume the generated `notio-fhir` types directly; never re-model FHIR here.
  A missing shape is a `notio-fhir-codegen` fix (`.claude/rules/codegen.md`).
- One operation, one module; the parameter set an operation accepts is what
  the version's `OperationDefinition` declares, and a version difference is a
  generated difference, never a hand-written conditional.
- Every failure is a typed error the server maps to an `OperationOutcome`;
  a client input error is never a panic.
- Tests cite the FHIR operation page or the SNOMED clause they assert.

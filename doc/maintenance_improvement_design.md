# Maintenance Improvement Design

## Scope

This maintenance cycle improves `rs-json` without adding product features. It
covers the owned encode-error API, the private `serde_json::RawValue`
compatibility boundary, CI extensibility, regression and fuzz tests, mutable
tree safety documentation, Rustdoc, and the bilingual project documentation.
It also adapts the directly affected downstream crates.

The repositories in scope are `rs-json`, `rs-ci`, `rs-datatype`, `rs-config`,
`rs-metadata`, and `rs-value`. `rs-http` and `rs-redact` are compatibility-only
consumers unless verification proves that a source change is required.

## Owned encode-error API

`JsonEncodeErrorSource<R, Q>` exposes the four owned sources of a
`JsonEncodeError<R, Q>`:

- `Budget(MeasuredBudgetError<R, Q>)`
- `InvalidRawJson(JsonSyntaxError)`
- `Serialize(JsonSerializationError)`
- `Write(std::io::Error)`

`JsonEncodeError::into_source` consumes the error and returns that enum. The
existing `kind` and `into_*` methods remain available for compatibility.
Downstream mappings use an exhaustive match on the new enum instead of pairing
`kind()` with a fallible extractor and an `expect` assertion.

## `RawValue` compatibility boundary

The private token used by `serde_json::RawValue` is owned by one crate-private
compatibility module. Both the general encode compatibility layer and the
`JsonValue` serializer import it from that module. This keeps the dependency
on a private upstream convention explicit and gives future `serde_json`
upgrades one audit point.

## Project-specific CI checks

`rs-ci` provides an optional root-level `project-ci-check.sh` hook. A missing
hook is a no-op. A present hook must be executable, otherwise CI fails with an
actionable error. The local full-check script and the reusable GitHub Actions
workflow each invoke the hook once from the project root with the resolved
toolchain environment.

`rs-json` uses the hook for documentation example tests and the ordinary tests
in `fuzz/Cargo.toml`, including the JSON number-contract suite. Its root
`ci-check.sh` delegates to `.rs-ci/ci-check.sh`, so the hook is not duplicated.

## Tests and safety documentation

Regression coverage includes decimal-width boundaries, all owned encode-error
variants, downstream error mappings, structured invalid-JSON diagnostics, and
the `rs-datatype` accounting path. Each fuzz target has a small checked-in
seed, and successful mutable-tree fuzz executions prove that every `secret`
field was removed.

The mutable traversal implementation is retained. Module-level safety notes
document the cursor, frame, stack, and node-identity invariants, and the
configured Miri tests continue to validate those boundaries.

## Documentation

The English and Simplified Chinese documentation remain semantically aligned.
The README files distinguish the released crates.io version from unreleased
0.8 development and describe Git/path installation. The user guides accurately
distinguish strict empty-input errors from normalization. Design documents
describe transport aggregation consistently, number-contract documents link
to their translation, and benchmark evidence is complete in both languages.
Rustdoc states `Option` semantics, return behavior, failure conditions, and the
current decoder terminology.

A bilingual changelog and a 0.3-to-0.8 migration guide record the public
transition. No `SECURITY.md` is introduced.

## Downstream boundary

`rs-datatype` calls `JsonTreeReader::account` directly and removes the no-op
accounting visitor. `rs-config`, `rs-metadata`, and `rs-value` adopt
`JsonEncodeErrorSource`. No unrelated downstream refactoring is included.

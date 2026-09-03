# `qubit-json` product requirements

[中文版](json_prd.zh_CN.md)

## Problem

Rust services handle two kinds of JSON: strict byte streams from controlled
protocols, and text-channel input that may contain a fence or a small amount
of explicitly allowed noise. Both kinds need resource limits. Callers also
need to distinguish budget, syntax, type, and output failures instead of
depending on overlapping historical error types.

## Product boundary

| Domain | Delivered capability | Responsibility excluded |
| --- | --- | --- |
| `decode` | Configurable normalization, strict decoding, and lexical admission | Guessing or repairing arbitrary damaged JSON |
| `encode` | Strict object-based encoder | Text repair or implicit sessions |
| `value` | Constructing and accounting for `Value` from Serde events | Replacing original-text syntax validation |
| `value::traverse` | Non-recursive reading, mutation, and complete-tree accounting | Rolling back completed mutable visitor changes |

Limits, budgets, and sessions belong to `qubit-budget`; this crate does not
redefine those general concepts.

## User contracts

### Normalized input

Users configure permitted normalization through
`NormalizingJsonDecodePolicy` and explicitly provide `JsonDecodeLimits` or a
`JsonDecodeSession` to `NormalizingJsonDecoder`. A policy must not carry a
budget, and the decoder must not create an implicit default budget. Diagnostics
are redacted by default; only an explicit `Detailed` policy retains
potentially input-derived details. Session-based calls preserve raw and
normalized input charges and commit value charges only after complete typed
decoding. One-shot normalization targets owned values; borrowed, seed-based,
and repeated materialization use the two-stage `NormalizedJsonDocument` API.
Preparation charges input once, while each document decode accounts and
commits value resources independently.

The decoder receives a complete `&str` or `&[u8]`. Limiting that slice does
not limit memory already allocated while an outer transport assembled it; the
outer read or body aggregation must be bounded separately.

### Strict text

Users pass a caller-owned session through objects: `JsonDecoder` provides
`decode_str`/`decode_utf8`, seed entry points, and validation; `JsonEncoder`
provides `to_vec`, `write_buffered`, and `write_incremental`. Strict input is
never repaired. Each document's accounting boundary is defined by its session
transaction. `JsonDecoder` defaults to `DiagnosticPolicy::Redacted`; callers
must explicitly select `Detailed` to retain input-derived sources.

The numeric product boundary is negative `i64`, non-negative `u64`, and finite
`f64` for fractional or exponential values. Identifiers above JavaScript's
safe integer range but within 64 bits are valid JSON numbers; a frontend must
use a precision-preserving parser or BigInt mapping. An `n` suffix is not
JSON. Wider integers and exact decimals use strings or explicit domain wire
types. `NumberBytes` limits token resources but does not alter numeric range.

### Value and tree

Users can use `AccountingJsonValueSeed` with a `JsonValueTransaction` to
construct `serde_json::Value`. `JsonTreeReader` and `JsonTreeMutator` traverse
materialized values without Rust recursion. The reader can account in an
existing caller transaction without callbacks or committing a transaction;
`JsonTreeBudgetTracker` reuses that path for repeated complete-tree
accounting. The mutator admits the complete input before callbacks. If input
admission fails, no callback runs. After callbacks succeed, it independently
measures and admits the complete output, without promising to roll back
business mutations already made.

## Error contract

Both decoder facades return the same generic `JsonDecodeError<R, Q>` and do
not expose a legacy compatibility layer. Public errors belong to their
domains:

1. `JsonDecodeError`, with stable kind, stage, accessors, and an exhaustive
   owned `JsonDecodeErrorSource` returned by `into_source()`.
2. `JsonEncodeError`, for strict budget, raw JSON, serialization, and writing.
3. `JsonSyntaxError`, for stable syntax reasons and positions.
4. `JsonTreeProcessError`, for reader budget and visitor failures.
5. `JsonTreeMutateError`, for mutator input budget, visitor, and output budget failures.

## Acceptance criteria

- Public areas are `decode`, `encode`, and `value`; tree APIs live under
  `value::traverse`, and shared implementation remains crate-private.
- All strict text operations go through decoder or encoder objects rather than
  public free functions.
- Session-aware decoding preserves input charges after failure, while staged
  value charges commit only after complete success.
- Value and tree APIs work independently on materialized JSON, and tree
  traversal does not depend on Rust call-stack depth.
- Documentation and examples describe the four current domains, the unified
  decode error model, and the `0.8` installation form.
- The dependency graph does not enable `serde_json/arbitrary_precision`; the
  old private Number marker remains an ordinary object key.

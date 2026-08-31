# `qubit-json` 64-bit number convergence design

[中文版](number_contract_design.zh_CN.md)

## Background

The historical implementation enabled serde_json `arbitrary_precision` and
recognized its private Number Serde marker. As a result, a legitimate object
such as `{"$serde_json::private::Number":"123"}` could become a number, making
the text structure, materialized value, and budget category disagree. The
same protocol could make a seed that did not recognize the marker materialize
a real large number as an object.

## Decision

The fundamental role of `qubit-json` is a resource-aware JSON boundary, not
an arbitrary-precision arithmetic library or a separate JSON parser. Keep
lexical admission, typed Serde decoding, budget/session/transaction handling,
and `serde_json::Value`; remove the entire private arbitrary-precision Number
protocol.

The public representation is fixed: negative integers use `i64`,
non-negative integers use `u64`, and fractional or exponential numbers use
finite `f64`. This supports Java `long` and the existing frontend
precision-preserving parser/BigInt path, but does not promise JavaScript
`Number` safety. Wider integers and exact decimals use strings or explicit
domain wire representations.

## Data flow and errors

```text
input bytes
  -> lexical syntax and budget admission
  -> integer/floating-point range validation
  -> serde_json deserializer without arbitrary_precision
  -> target type
  -> commit staged value budget after complete success
```

`NumberBytes` admission precedes range validation, preserving resource-failure
priority. Integer overflow returns `IntegerOutOfRange`; floating-point
overflow returns `FloatOutOfRange`. Errors carry only safe positions and do
not copy the complete token. The encoder performs symmetric checks for
`i128/u128` and returns a serialization error rather than truncating or
silently converting to a string.

## Private protocol boundary

Production code does not recognize `$serde_json::private::Number`; that key
is always an ordinary object key. `RawValue` is a separate serde_json
integration with public uses, so it remains supported and separately tested.
Compact JSON lexeme length for finite floats is measured through serde_json's
public `CompactFormatter` interface rather than coupling directly to its
private formatting dependency.

## Seeds and materialized values

`AccountingJsonValueSeed` can observe only decoded Serde events. It returns a
recoverable error when `i128/u128` exceed the combined `i64/u64` range, but it
cannot verify the original number token, lexical budget, or text-level range.
Those guarantees must be provided by `JsonDecoder`.

## Acceptance invariants

- An object using the marker name remains an object and is charged as
  object/key/string data.
- Integers above `u64::MAX` or below `i64::MIN` do not enter typed
  `serde_json` decoding.
- The published dependency graph does not enable
  `serde_json/arbitrary_precision`.
- Production code has no Number-marker recognition, generation, or reserved-key
  logic.
- The bilingual README, user guides, number contract, rustdoc, design, and
  dependency-maintenance documents remain semantically aligned.
- No custom parser, new `JsonValue` model, or unrelated budget/tree rewrite is
  introduced.

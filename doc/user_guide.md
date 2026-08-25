# Qubit JSON user guide

This guide targets Rust 1.94+ applications using `qubit-json` 0.8. It is for
services, configuration readers, and data pipelines that must admit JSON under
caller-owned resource limits while keeping Serde's data model. It does not
replace `serde_json` or impose application schemas.

[中文版](user_guide.zh_CN.md) · [README](../README.md) · [API documentation](https://docs.rs/qubit-json/0.8.0/qubit_json/)

## Purpose and audience

Use this crate at a trust boundary where input size, nesting, collection size,
number-token size, decoded values, or output bytes need explicit limits. Keep
schema validation, authorization, identifier policy, and decimal precision in
the application layer after JSON admission.

## Conceptual model

`qubit-budget` owns resource identities, limits, transactions, and sessions.
`qubit-json` supplies four public areas:

- `decode` admits strict JSON text or performs explicitly configured text
  normalization before strict admission.
- `encode` serializes values to budgeted JSON output.
- `value` builds or validates materialized `serde_json::Value` trees from Serde
  events.
- `value::traverse` reads or mutates an existing tree without Rust recursion.

Strict decoders and encoders are stateful objects. A caller may use
`owned(limits)` for an isolated run or pass a `JsonDecodeSession`/
`JsonEncodeSession` to accumulate charges across calls. Decoded-value and
buffered-output charges are staged and commit at their documented success
boundary; input charges remain visible after a failed decode attempt.

## Scenario: bounded JSON at an HTTP boundary

Suppose an endpoint accepts a JSON document containing an identifier and a
small payload. The service wants to reject oversized requests before building
application state, while still accepting the full unsigned 64-bit identifier
range.

### Installation and minimal configuration

```toml
[dependencies]
qubit-json = "0.8"
qubit-budget = { version = "0.3", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Core workflow

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonResource};
use qubit_json::decode::JsonDecoder;

let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
    .max_input_bytes(4096)
    .max_number_bytes(20)
    .build();
let mut decoder = JsonDecoder::owned(limits);
let value: serde_json::Value =
    decoder.decode_str(r#"{"id":18446744073709551615,"ok":true}"#)?;
assert_eq!(value["id"], serde_json::json!(u64::MAX));
# Ok::<(), qubit_json::decode::JsonDecodeError<JsonResource>>(())
```

The observable result is an admitted `serde_json::Value`; a limit breach is a
`JsonDecodeError::Budget` before the typed value is committed. A syntax or
number-range problem is reported as `JsonDecodeError::Syntax`, while a target
type mismatch is `JsonDecodeError::Deserialize`.

For output, create `JsonEncoder::owned(JsonEncodeLimits::...)` and call
`to_vec`, `write_buffered`, or `write_incremental`. Buffered output is committed
only when the complete byte sequence is ready; incremental output may leave an
accepted prefix in the writer when the writer fails.

## Advanced usage

Use `NormalizingJsonDecoder` only when the boundary explicitly permits the
configured transformations (BOM, surrounding whitespace, one JSON Markdown
fence, or control-character escaping). Its policy is independent from
`JsonDecodeLimits`; pass limits to `owned` or a session to `from_session`.

Use `JsonValueSeed` when another Serde deserializer owns the input. It charges
decoded events but cannot inspect original number lexemes, `NumberBytes`, or
text-level integer range. Use `JsonDecoder` for those guarantees.

For an existing value, `JsonTreeReader::account` stages complete-tree charges
in the caller's transaction without invoking a visitor. `JsonTreeMutator`
first admits the original tree, then runs in-place visitor callbacks, and
finally admits the mutated tree. Its errors are
`JsonTreeMutateError::InputBudget`, `::Visitor`, and `::OutputBudget`; visitor
and output failures retain mutations already made to the value. A visitor can
return `JsonTreeControl::SkipSubtree` to skip descendant callbacks, but final
output accounting still covers every resulting descendant.

## Errors and diagnostics

The public error domains are `NormalizingJsonDecodeError`, `JsonDecodeError`,
`JsonEncodeError`, `JsonSyntaxError`, `JsonTreeProcessError`, and
`JsonTreeMutateError`. Match the domain before deciding whether to retry, return
a client error, or log a diagnostic. Normalizing diagnostics can retain
input-derived details; use `DiagnosticPolicy::Detailed` only at trusted
boundaries and redact untrusted logs by default.

The numeric contract is independent from resource limits: negative integers fit
`i64`, non-negative integers fit `u64`, and fractional/exponential values must
be finite `f64`. Values outside those ranges or exact decimals that must avoid
binary rounding should use a string or explicit domain representation. See the
[number contract](number_contract.md).

## Troubleshooting

- A `Budget` error: inspect the matching limit (input bytes, number bytes,
  depth, nodes, collection sizes, key/string bytes, or output bytes) and keep
  the session alive only if cumulative accounting is intended.
- A `Syntax` error: validate the original bytes and check the reported reason,
  offset, line, and column; normalization never invents missing JSON syntax.
- A `Deserialize` error: the JSON was admitted but does not match the target
  type; fix the payload or the target schema separately.
- An unexpected tree mutation after an error: `JsonTreeMutator` is incremental;
  visitor and output-budget failures do not roll back prior mutations.

## Limitations and best practices

Set finite limits at every untrusted boundary, including input/output bytes,
depth, nodes, collection sizes, key/string bytes, and number bytes. Do not use
an unlimited session merely to avoid choosing limits. Keep application
validation separate from resource admission, and treat browser `BigInt`
handling as a wire-format concern for integers above JavaScript's safe range.

## Further reading

- [English README](../README.md) · [中文 README](../README.zh_CN.md)
- [中文用户手册](user_guide.zh_CN.md)
- [JSON number contract](number_contract.md)
- [API documentation](https://docs.rs/qubit-json/0.8.0/qubit_json/)

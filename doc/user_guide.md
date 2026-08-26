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
`JsonDecodeError` whose `kind()` is `JsonDecodeErrorKind::Budget` before the
typed value is committed. Syntax and number-range problems use `InvalidJson`;
target type mismatches use `Deserialize`. Use `stage()` to distinguish input,
normalization, admission, parsing, top-level checking, and materialization.

For output, create `JsonEncoder::owned(JsonEncodeLimits::...)` and call
`to_vec`, `write_buffered`, or `write_incremental`. Buffered output is committed
only when the complete byte sequence is ready; incremental output may leave an
accepted prefix in the writer when the writer fails.

## Advanced usage

Use `NormalizingJsonDecoder` only when the boundary explicitly permits the
configured transformations (BOM, surrounding whitespace, one JSON Markdown
fence, or control-character escaping). Its policy is independent from
`JsonDecodeLimits`; pass limits to `owned` or a session to `new`.

### Prepared normalized documents

The one-shot normalizing methods return owned targets because temporary
normalized text cannot outlive the call. For borrowing, seed-driven decoding,
or repeated materialization, prepare a document explicitly:

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_json::decode::{NormalizingJsonDecodePolicy, NormalizingJsonDecoder};

let mut decoder = NormalizingJsonDecoder::owned(
    NormalizingJsonDecodePolicy::lenient(),
    JsonDecodeLimits::default(),
);
let document = decoder.prepare_str("  \"borrowed\"  ")?;
let first: &str = decoder.decode_document(&document)?;
let second: &str = decoder.decode_document(&document)?;
assert_eq!((first, second), ("borrowed", "borrowed"));
# Ok::<(), qubit_json::decode::JsonDecodeError>()
```

`prepare_str`/`prepare_utf8` immediately commit raw and normalized input
charges. Each successful `decode_document`, `decode_document_seed`, root-typed
document decode, or `validate_document` commits a separate value charge; a
failed materialization rolls back only that attempt's value charges. The
document is detached and may be decoded by another compatible decoder.
Borrowing follows Serde's representation rules: unescaped JSON strings can
borrow from the document, while escaped strings require owned targets.

Use `JsonValueSeed` when another Serde deserializer owns the input. It charges
decoded events but cannot inspect original number lexemes, `NumberBytes`, or
text-level integer range. Use `JsonDecoder` for those guarantees.
Use `decode_object_str`/`decode_object_utf8` or the corresponding array methods
when the boundary also requires a specific top-level container.

### Duplicate object keys

Strict admission does not add a global unique-key rule. Duplicate-key behavior
belongs to the requested Serde target: `serde_json::Value` and
`serde_json::Map` use last-key-wins semantics, while many derived structs
reject duplicate known fields. Choose the target that expresses the document
contract instead of assuming that `strict` includes key uniqueness.

Compose strict text admission with `DuplicateKeyRejectingJsonValue` when every
object in a dynamic document must have unique keys:

```rust
use qubit_json::decode::JsonDecoder;
use qubit_json::value::DuplicateKeyRejectingJsonValue;

let input = r#"{"role":"user","role":"admin"}"#;

let mut ordinary_decoder = JsonDecoder::unlimited();
let ordinary: serde_json::Value = ordinary_decoder.decode_str(input)?;
assert_eq!(ordinary["role"], "admin");

let mut unique_key_decoder = JsonDecoder::unlimited();
let error = unique_key_decoder
    .decode_str::<DuplicateKeyRejectingJsonValue>(input)
    .expect_err("duplicate object keys must be rejected");
assert!(error.to_string().contains("deserialization failed"));
# Ok::<(), qubit_json::decode::JsonDecodeError<
#     qubit_budget::json::JsonResource,
# >>(())
```

The wrapper validates nested objects recursively and can also be the target of
`NormalizingJsonDecoder` when the configured text cleanup is part of the input
contract. `DuplicateKeyRejectingJsonValueSeed` provides the corresponding seed
for a caller-owned Serde deserializer.

For an existing value, `JsonTreeReader::account` stages complete-tree charges
in the caller's transaction without invoking a visitor. `JsonTreeMutator`
first admits the original tree, then runs in-place visitor callbacks, and
finally admits the mutated tree. Its errors are
`JsonTreeMutateError::InputBudget`, `::Visitor`, and `::OutputBudget`; visitor
and output failures retain mutations already made to the value. A visitor can
return `JsonTreeControl::SkipSubtree` to skip descendant callbacks, but final
output accounting still covers every resulting descendant.

## Errors and diagnostics

Strict and normalizing decoders return the same generic `JsonDecodeError`.
Branch on `JsonDecodeErrorKind` through `kind()`, then use `stage()`,
`budget_error()`, `syntax_error()`, top-level accessors, or UTF-8 accessors for
the applicable structured details. Input-derived sources are retained only by
`DiagnosticPolicy::Detailed`; enable it only at trusted boundaries and keep
untrusted logs redacted. The other domains expose `JsonEncodeError`,
`JsonSyntaxError`, `JsonTreeProcessError`, and `JsonTreeMutateError`.

The numeric contract is independent from resource limits: negative integers fit
`i64`, non-negative integers fit `u64`, and fractional/exponential values must
be finite `f64`. Values outside those ranges or exact decimals that must avoid
binary rounding should use a string or explicit domain representation. See the
[number contract](number_contract.md).

## Troubleshooting

- A `Budget` error: inspect the matching limit (input bytes, number bytes,
  depth, nodes, collection sizes, key/string bytes, or output bytes) and keep
  the session alive only if cumulative accounting is intended.
- An `InvalidJson` error: validate the original bytes and check the reported reason,
  offset, line, and column; normalization never invents missing JSON syntax.
- A `Deserialize` error: the JSON was admitted but does not match the target
  type; fix the payload or the target schema separately. This also includes a
  `serde_json` materialization recursion-guard failure: lexical validation uses
  an explicit stack and can admit a document that the typed deserializer will
  not materialize. If the target is `DuplicateKeyRejectingJsonValue`, a
  duplicate object key is also reported at this boundary.
- An unexpected tree mutation after an error: `JsonTreeMutator` is incremental;
  visitor and output-budget failures do not roll back prior mutations.

## Limitations and best practices

Set finite limits at every untrusted boundary, including input/output bytes,
depth, nodes, collection sizes, key/string bytes, and number bytes. Do not use
an unlimited session merely to avoid choosing limits. Keep application
validation separate from resource admission, and treat browser `BigInt`
handling as a wire-format concern for integers above JavaScript's safe range.
Keep Serde's depth guard enabled as well. Disabling it only moves deeply nested
untrusted inputs onto the Rust call stack and does not make arbitrary target
deserializers safe.

## Further reading

- [English README](../README.md) · [中文 README](../README.zh_CN.md)
- [中文用户手册](user_guide.zh_CN.md)
- [JSON number contract](number_contract.md)
- [API documentation](https://docs.rs/qubit-json/0.8.0/qubit_json/)

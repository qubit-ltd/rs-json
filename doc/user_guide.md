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

Strict decoders and encoders are stateful objects. Use `with_limits(limits)`
with a fresh owned session, or pass a `JsonDecodeSession`/`JsonEncodeSession` to
`new(session)` when charges must accumulate across calls. Decoded-value and
buffered-output charges
are staged in transactions and commit at their documented success boundary;
input charges remain visible in the session after a failed decode attempt.

## Scenario: bounded JSON at an HTTP boundary

Suppose an endpoint receives a JSON request body containing an identifier and a
small payload. Success means accepting the complete unsigned 64-bit identifier
range, rejecting an oversized body before a decoded value is committed, and
then handing only admitted data to application validation.

### Installation and minimal configuration

```toml
[dependencies]
qubit-json = "0.8"
qubit-budget = { version = "0.4", features = ["json"] }
serde_json = "1.0"
```

Add `serde = { version = "1.0", features = ["derive"] }` when the application
decodes into derived types.

### Core workflow

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;

fn main() -> Result<(), JsonDecodeError<JsonResource>> {
    let limits = JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
        .max_input_bytes(4096)
        .max_depth(32)
        .max_nodes(256)
        .max_sequence_items(64)
        .max_map_entries(64)
        .max_key_bytes(128)
        .max_string_bytes(2048)
        .max_number_bytes(20)
        .max_payload_bytes(4096)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);

    let request_body = br#"{"id":18446744073709551615,"ok":true}"#;
    let value: serde_json::Value = decoder.decode_utf8(request_body)?;
    assert_eq!(value["id"], serde_json::json!(u64::MAX));

    let small_limits = limits.into_builder().max_input_bytes(8).build();
    let mut small_decoder = JsonDecoder::with_limits(small_limits);
    let error = small_decoder
        .decode_utf8::<serde_json::Value>(br#"{"ok":true}"#)
        .expect_err("the request body must exceed eight bytes");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.raw_input_bytes(), 11);
    Ok(())
}
```

The observable success result is an admitted `serde_json::Value`. The failure
branch returns `JsonDecodeErrorKind::Budget` and preserves the measured raw
input length without committing a decoded value. This lets an HTTP adapter map
stable error categories to responses without matching private parser details.

For output, create `JsonEncoder::with_limits(JsonEncodeLimits::...)` and call
`to_vec`, `write_buffered`, or `write_incremental`. Buffered mode finishes
serialization and budget checks before touching the writer, and commits
accounting only after the complete write succeeds; an I/O failure can still
leave partial bytes in the external writer. Incremental mode retains accepted
prefixes when a later serialization, budget, or writer operation fails.

The next step is to deserialize the admitted document into an application type
and apply schema, authorization, and identifier rules. Continue below when the
boundary also needs normalization, repeated decoding, unique object keys, or
tree processing.

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
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;

fn main() -> Result<(), JsonDecodeError> {
    let limits = JsonDecodeLimits::builder()
        .max_input_bytes(1024)
        .max_normalized_input_bytes(1024)
        .max_depth(16)
        .max_nodes(64)
        .max_sequence_items(32)
        .max_map_entries(32)
        .max_key_bytes(128)
        .max_string_bytes(512)
        .max_number_bytes(32)
        .max_payload_bytes(1024)
        .build();
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::lenient(),
        limits,
    );
    let document = decoder.prepare_str("  \"borrowed\"  ")?;
    let first: &str = decoder.decode_document(&document)?;
    let second: &str = decoder.decode_document(&document)?;
    assert_eq!((first, second), ("borrowed", "borrowed"));
    Ok(())
}
```

`prepare_str`/`prepare_utf8` immediately commit raw and normalized input
charges. Each successful `decode_document`, `decode_document_seed`, root-typed
document decode, or `validate_document` commits a separate value charge; a
failed materialization rolls back only that attempt's value charges. The
document is detached and may be decoded by another compatible decoder.
Borrowing follows Serde's representation rules: unescaped JSON strings can
borrow from the document, while escaped strings require owned targets.

Use `AccountingJsonValueSeed` when another Serde deserializer owns the input. It charges
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
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json::value::DuplicateKeyRejectingJsonValue;

fn main() -> Result<(), JsonDecodeError<JsonResource>> {
    let input = r#"{"role":"user","role":"admin"}"#;
    let limits = JsonDecodeLimits::builder()
        .max_input_bytes(1024)
        .max_depth(16)
        .max_nodes(64)
        .max_sequence_items(16)
        .max_map_entries(16)
        .max_key_bytes(64)
        .max_string_bytes(256)
        .max_number_bytes(32)
        .max_payload_bytes(1024)
        .build();

    let mut ordinary_decoder = JsonDecoder::with_limits(limits);
    let ordinary: serde_json::Value = ordinary_decoder.decode_str(input)?;
    assert_eq!(ordinary["role"], "admin");

    let mut unique_key_decoder = JsonDecoder::with_limits(limits);
    let error = unique_key_decoder
        .decode_str::<DuplicateKeyRejectingJsonValue>(input)
        .expect_err("duplicate object keys must be rejected");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    Ok(())
}
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
Branch on the stable category returned by `kind()`:

| `JsonDecodeErrorKind` | Meaning | Structured details |
| --- | --- | --- |
| `Budget` | A configured resource limit rejected a measurement | `budget_error()`, `raw_input_bytes()`, `normalized_input_bytes()` |
| `EmptyInput` | Input was empty at the active strict or normalization boundary | `stage()`, input byte counts |
| `InvalidUtf8` | A byte input was not valid UTF-8 | `utf8_valid_up_to()`, `utf8_error_len()` |
| `InvalidJson` | Syntax or the numeric contract was invalid | `syntax_error()`, `line()`, `column()` |
| `UnexpectedTopLevel` | An object/array-specific API received the wrong root kind | `expected_top_level()`, `actual_top_level()` |
| `Deserialize` | An admitted document could not materialize as the requested type | `line()`, `column()`, and a detailed source when enabled |

`JsonValueEncoder` uses a separate, privacy-safe error model. Use
`JsonValueEncodeError::category()` for recovery policy and `kind()` when the
exact reason matters. Convenience predicates cover the common groups, while
typed accessors expose only applicable details:

| Category | Representative kinds | Convenience/details |
| --- | --- | --- |
| `Number` | `IntegerOutOfRange`, `NonFiniteFloat`, `InvalidNumberRepresentation` | `is_number_error()`, `integer_signedness()` |
| `ObjectKey` | `UnsupportedMapKey`, `DuplicateObjectKey` | `is_map_key_error()`, `map_key_kind()` |
| `RawValue` | `InvalidRawValue` | `is_raw_value_error()` |
| `Capacity` | `CollectionLengthOverflow` | `collection_kind()` |
| `SerializerContract` | `InvalidSerializerState`, `DisplayFormattingFailed` | `is_serializer_contract_error()`, `serializer_state_error()` |
| `Custom` | `CustomSerialization` | Opaque by design: arbitrary serializer text is not retained |

`stage()` identifies the public processing boundary precisely:

| `JsonDecodeStage` | Boundary |
| --- | --- |
| `Input` | Charge raw input bytes |
| `DecodeText` | Validate byte input as UTF-8 |
| `Normalize` | Transform text or charge normalized bytes |
| `Admission` | Admit decoded-value resources |
| `Parse` | Validate JSON syntax and numeric range |
| `TopLevelCheck` | Enforce an object or array root |
| `Deserialize` | Materialize the requested Rust type |

Input-derived sources are retained only by `DiagnosticPolicy::Detailed`.
Configure strict decoding with
`JsonDecoder::with_diagnostic_policy(DiagnosticPolicy::Detailed)`; configure
normalizing decoding through `NormalizingJsonDecodePolicyBuilder`. Enable
detailed diagnostics only at trusted boundaries and keep untrusted logs
redacted. The other domains expose `JsonEncodeError`, `JsonSyntaxError`,
`JsonTreeProcessError`, and `JsonTreeMutateError`.

The numeric contract is independent from resource limits: negative integers fit
`i64`, non-negative integers fit `u64`, and fractional/exponential values must
be finite `f64`. Values outside those ranges or exact decimals that must avoid
binary rounding should use a string or explicit domain representation. See the
[number contract](number_contract.md).

## Troubleshooting

- A `Budget` error: inspect the matching limit (input bytes, number bytes,
  depth, nodes, collection sizes, key/string bytes, or output bytes) and keep
  the session alive only if cumulative accounting is intended.
- An `EmptyInput` error: check whether the body was empty before decoding or
  became empty after the configured whitespace, BOM, or fence handling.
- An `InvalidUtf8` error: inspect `utf8_valid_up_to()` and `utf8_error_len()`,
  and reject or repair the byte transport before JSON parsing.
- An `InvalidJson` error: validate the original bytes and check the reported reason,
  offset, line, and column; normalization never invents missing JSON syntax.
- An `UnexpectedTopLevel` error: compare `expected_top_level()` with
  `actual_top_level()`, then either fix the payload or select a decoder method
  whose root contract matches it.
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

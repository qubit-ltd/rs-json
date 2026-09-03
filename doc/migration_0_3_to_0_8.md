# Migrating from 0.3 to 0.8

[简体中文](migration_0_3_to_0_8.zh_CN.md)

Version 0.8 is a redesign, not a drop-in upgrade. It separates strict JSON
admission, controlled text normalization, encoding, materialized values, and
traversal. Budget configuration is now explicit at the trust boundary.

## Dependency

```toml
[dependencies]
qubit-json = "0.8"
qubit-budget = { version = "0.4", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
```

## Rename and module map

| 0.3 | 0.8 |
| --- | --- |
| `qubit_json::LenientJsonDecoder` | `qubit_json::decode::NormalizingJsonDecoder` |
| `qubit_json::JsonDecodeOptions` | `qubit_json::decode::NormalizingJsonDecodePolicy` plus `JsonDecodeLimits` |
| `qubit_json::JsonDecodeError*` | `qubit_json::decode::JsonDecodeError*` |
| `qubit_json::JsonTopLevelKind` | `qubit_json::decode::JsonRootKind` |
| strict option preset | `qubit_json::decode::JsonDecoder` |

Decoder methods now take `&mut self` because a decoder owns cumulative
accounting state. Construct a new decoder per independent boundary or reuse it
deliberately for cumulative limits.

## Normalizing input

Before:

```rust,ignore
use qubit_json::LenientJsonDecoder;

let decoder = LenientJsonDecoder::default();
let value = decoder.decode_value("```json\n{\"ok\":true}\n```")?;
```

After:

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonResource};
use qubit_json::decode::{NormalizingJsonDecodePolicy, NormalizingJsonDecoder};

let policy = NormalizingJsonDecodePolicy::default();
let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
    .max_input_bytes(4096)
    .max_depth(32)
    .max_nodes(256)
    .build();
let mut decoder = NormalizingJsonDecoder::with_limits(policy, limits);
let value: serde_json::Value =
    decoder.decode_str("```json\n{\"ok\":true}\n```")?;
assert_eq!(value["ok"], true);
# Ok::<(), qubit_json::decode::JsonDecodeError<JsonResource>>(())
```

Use `prepare_str` or `prepare_utf8` when a decoded value must borrow from the
normalized document, when using a Serde seed, or when decoding the same
normalized text more than once.

## Strict JSON

Use `JsonDecoder` when presentation cleanup is not part of the input contract.
There is no implicit unlimited default: call `with_limits` at untrusted
boundaries, or spell `unlimited()` only for trusted/already-admitted input.
Strict raw empty input is `InvalidJson`; `EmptyInput` means configured
normalization produced no document.

## Errors

Decode errors are generic over the resource and quantity types and expose
stable `kind`, `stage`, budget, syntax, location, and diagnostic-policy data.
Adapters that own an error can exhaustively match `JsonDecodeErrorSource`
after calling `JsonDecodeError::into_source`.
Diagnostics are redacted by default: stable classifications and source
coordinates remain available, but offending bytes and other input content do
not. Detailed Serde sources require an explicit `DiagnosticPolicy::Detailed`
choice. `JsonSyntaxErrorReason::UnexpectedByte` is now a content-free unit
variant; inspect caller-owned input at the reported offset when a trusted
diagnostic needs the byte.

Encoding uses `JsonEncodeError`. Consumers that move errors into another model
should exhaustively match `JsonEncodeError::into_source()` rather than combine
`kind()` with an optional `into_*` extractor.

## New domains

- `encode::JsonEncoder` performs strict encoding with value and output budgets.
- `value::JsonValueEncoder` builds strict `serde_json::Value` instances from
  Serde events.
- `value::DuplicateKeyRejectingJsonValue` enforces unique object keys.
- `value::traverse` provides iterative readers and in-place mutators with
  transactional accounting.

Review the [user guide](user_guide.md), [number contract](number_contract.md),
and [design](json_design.md) before migrating a security-sensitive boundary.

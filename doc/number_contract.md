# JSON number contract

[简体中文](number_contract.zh_CN.md)

This document defines the numeric representation accepted by `qubit-json`.
It is a deliberate implementation contract, not an additional JSON grammar.
RFC 8259 does not set a maximum number length, but permits implementations to
limit numeric range and precision.

## Default strict model

Unless an entry point explicitly selects another model, ordinary strict and
normalizing text decoding uses `Integer64`. The ranges below describe that
default model, not every decoder.

## Accepted values

| JSON token | Contract |
| --- | --- |
| Negative integer | `i64::MIN..=-1` |
| Non-negative integer | `0..=u64::MAX` |
| Fraction or exponent | Must parse to a finite `f64` |

The boundaries are inclusive. `-9223372036854775808` and
`18446744073709551615` are accepted; the adjacent values outside that range
produce `IntegerOutOfRange`. A fraction or exponent that overflows binary64,
such as `1e400`, produces `FloatOutOfRange`. Underflow and normal decimal
rounding follow `f64`/serde_json behavior.

Strict encoding has its own range checks: all `i64` and `u64` values are
accepted. Serde `i128` is accepted only if it fits `i64`, or is non-negative
and fits `u64`; `u128` must fit `u64`. Values outside the range fail instead
of being truncated or implicitly converted to strings. Non-finite floats are
rejected. A decode fallback does not change this encoding contract.

## Select a text-decoding number model

Ordinary decoding defaults to `JsonDecodeNumberModel::Integer64`. The optional
`Integer64WithFiniteFallback` model only changes out-of-range integer tokens:
it admits one when conversion to a finite `f64` succeeds. Ordinary decimal and
exponent tokens must be finite in either model. `JcsDecoder` is a separate
contract: it converts every number, including values inside the integer range,
to finite binary64, as required by JCS.

Fallback is available only through decode entry points that explicitly expose
number-model selection. It is not automatically enabled by normalizing decode
or by ordinary slice methods.

| Entry point | In-range integers | Out-of-range integer token | Decimal/exponent token |
| --- | --- | --- | --- |
| Default `Integer64` | `i64`/`u64` | Rejected | Finite `f64` |
| `Integer64WithFiniteFallback` | `i64`/`u64` | Accepted as finite `f64`, with possible rounding | Finite `f64` |
| `JcsDecoder` | Converted to `f64` | Converted to finite `f64`, with possible rounding | Finite `f64` |

For example, `18446744073709551617` is outside `u64`: the default model
rejects it, while the fallback model can represent it as the rounded finite
binary64 value `18446744073709551616`. `1e400` remains rejected because it is
not finite. Model selection admits a number token; it does not guarantee that
every target type or application schema accepts the resulting value.

Validated owned-value decoding selects the model in its options:

```rust
use qubit_json::decode::{
    JsonDecodeNumberModel, JsonDecoder, JsonNumberPolicy,
    JsonValidatedDecodeOptions,
};

let options = JsonValidatedDecodeOptions::builder()
    .number_model(JsonDecodeNumberModel::Integer64WithFiniteFallback)
    .build();
let mut decoder = JsonDecoder::unlimited();
let value = decoder
    .decode_unique_validated_str("18446744073709551617", &options,
        |_, _| Ok::<_, std::convert::Infallible>(JsonNumberPolicy::Preserve))
    .unwrap();
assert_eq!(value.as_value().as_f64().unwrap(), 18446744073709551616_f64);
```

The input-stream typed entry point also accepts an explicit model:

```rust
use qubit_json::decode::{JsonDecodeNumberModel, JsonDecoder};
use serde_json::Value;
use std::io::Cursor;

let mut decoder = JsonDecoder::unlimited();
let mut input = Cursor::new(b"18446744073709551617".to_vec());
let value: Value = decoder
    .decode_input_with_number_model(
        &mut input,
        JsonDecodeNumberModel::Integer64WithFiniteFallback,
    )
    .unwrap();
assert_eq!(value.as_f64().unwrap(), 18446744073709551616_f64);

let mut input = Cursor::new(b"1e400".to_vec());
assert!(decoder
    .decode_input_with_number_model::<Value, _>(
        &mut input,
        JsonDecodeNumberModel::Integer64WithFiniteFallback,
    )
    .is_err());
```

## Resource budgets are separate

`NumberBytes` limits the lexical byte length of the original number token. It
does not define numeric precision or range. During text decoding the token is
first admitted to the configured budget, then range-checked. If both checks
would fail, the budget failure is returned first.

## JavaScript clients

The contract intentionally accepts integers above `Number.MAX_SAFE_INTEGER`
because existing Java services use 64-bit `long` identifiers. A browser client
must use a JSON parser that preserves integer text and converts unsafe values
to `BigInt` (or strings). The `n` suffix belongs to JavaScript source code;
`123n` is not valid JSON and must never appear on the wire.

## JCS raw-text decoding

With the `jcs` feature enabled, `JcsDecoder` uses a binary64 number contract
separate from the ordinary paths. Every JSON number, including integer
lexemes, is converted to `f64` before constructing `serde_json::Number`.
Thus `9007199254740993` rounds to `9007199254740992`; finite values may
underflow to zero, and negative zero retains its sign bit in the decoded value.
Numbers that overflow to a non-finite value, such as `1e400`, are rejected.
The original exact decimal digits of an integer cannot be recovered.

Use JSON strings for IDs, amounts, or other decimal values that must retain
every digit. Ordinary `JsonDecoder`, normalizing decoding, and the `RawValue`
protocol use the ordinary strict contract; only an explicit `JcsDecoder` uses
the JCS binary64 model. JCS decoding still measures
`NumberBytes` from the original token and checks that budget before its binary64
range check. These budgets measure resources; they are not exact heap-memory
limits.

JCS and unique-key `Value` construction use the shared builder and have no
separate fixed container cap. The root-inclusive configured `max_depth` is the
depth admission limit. Legacy `DepthLimit` and
`JcsDecodeError::MaterializationDepthLimit` reasons/variants remain for source
compatibility; the current shared builder reports configured depth rejection
as a budget error. `JsonValueEncoder` uses the same configured builder contract
when projecting a genuine `serde_json::value::RawValue`. A `JsonValueEncoder` `InvalidRawValue` error
denotes rejection by the strict materialization contract, which can include
duplicate keys, numeric constraints, or depth as well as invalid syntax.

## Exact and wider values

Use a JSON string or an explicit domain representation for integers below
`i64::MIN` or above `u64::MAX`. Use a decimal string or a coefficient/scale
object for money and other exact decimals. A bare fractional JSON number has
binary64 semantics and is not an exact decimal wire format.

## Already materialized trees and Serde seeds

The numeric rule depends on the entry point:

- `JsonDecoder::decode_unique_str`, `decode_unique_utf8`, and `decode_unique_seed_utf8` first admit the source document and construct a unique-key tree using the strict integer/finite-f64 contract, then deserialize canonical `Number` values as native Serde numeric events. This preserves numeric types under `arbitrary_precision` without assigning special meaning to object keys. Results are owned; configured root-inclusive `max_depth` controls construction, with no separate fixed container cap. Source value usage commits only after the target succeeds. The intermediate tree adds memory overhead, and target-deserialization errors have no original source coordinates. The target's own Serde semantics still apply; choose `DuplicateKeyRejectingJsonValue` when literal private-field objects must be preserved.
- Text decoding, normalizing decoding, ordinary serialization, and native integer Serde events use the ordinary contract above. `i128`/`u128` events are explicitly rejected unless representable as `i64` or `u64` as appropriate.
- `AccountingJsonValueSeed` and `DuplicateKeyRejectingJsonValueSeed` see decoded Serde events, not source text, so neither can inspect the original number lexeme. `AccountingJsonValueSeed` enforces the native 64-bit integer range and applies its `NumberBytes` budget to the compact display length of the constructed `Number`; that length may differ from the source token length. `DuplicateKeyRejectingJsonValueSeed` checks duplicate keys and does not itself apply resource budgets. If `arbitrary_precision` presents a number as a map, either seed cannot distinguish that from a real object with the same shape; it keeps the event as an object. Use a controlled text/RawValue/JCS entry point when provenance matters.
- `JsonTreeReader` operates on an existing `serde_json::Value`. It measures an existing Number by the byte length of its current compact display representation. It does not convert the number to `f64` or impose the ordinary text encoder range. With `arbitrary_precision`, a stored `1e400` can be accounted without panic, and a 39-digit integer counts as 39 bytes. Ordinary `JsonEncoder` and `JcsEncoder` still reject numbers outside their own contracts.

The crate does not enable serde_json's `arbitrary_precision` feature itself. If another dependency unifies that feature, the encoder recognizes serde_json's exact private Number struct protocol: one string payload delivered by `serialize_str` or one `collect_str`. It validates the payload with the shared JSON number scanner before writing or constructing a Value. A normal object with a key named `$serde_json::private::Number` remains an object. The separate `RawValue` private protocol is also supported and validated. Malformed private protocol shapes return `JsonSerializationErrorKind::InvalidSerializerState` with `JsonSerializerStateError::InvalidNumberProtocol` or `InvalidRawValueProtocol`; diagnostic text does not expose the payload.

JCS remains independent: raw text uses finite binary64 for every number and rejects overflow such as `1e400`; an already materialized wide `Number` under `arbitrary_precision` does not make `JcsEncoder` accept it.

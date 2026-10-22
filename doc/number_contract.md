# JSON number contract

[简体中文](number_contract.zh_CN.md)

This document defines the numeric representation accepted by `qubit-json`.
It is a deliberate implementation contract, not an additional JSON grammar.
RFC 8259 does not set a maximum number length, but permits implementations to
limit numeric range and precision.

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

Encoding is symmetric. All `i64` and `u64` values are accepted. Serde `i128`
is accepted only if it fits `i64`, or is non-negative and fits `u64`; `u128`
must fit `u64`. Values outside the range fail instead of being truncated or
implicitly converted to strings. Non-finite floats are rejected.

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

## Exact and wider values

Use a JSON string or an explicit domain representation for integers below
`i64::MIN` or above `u64::MAX`. Use a decimal string or a coefficient/scale
object for money and other exact decimals. A bare fractional JSON number has
binary64 semantics and is not an exact decimal wire format.

## Already materialized trees and Serde seeds

The numeric rule depends on the entry point:

- Text decoding, normalizing decoding, ordinary serialization, and native integer Serde events use the ordinary contract above. `i128`/`u128` events are explicitly rejected unless representable as `i64` or `u64` as appropriate.
- `AccountingJsonValueSeed` and `DuplicateKeyRejectingJsonValueSeed` see decoded Serde events, not source text. They enforce the native 64-bit integer range but cannot inspect the original token or enforce `NumberBytes`. If `arbitrary_precision` presents a number as a map, the seed cannot distinguish that from a real object with the same shape; it keeps the event as an object. Use a controlled text/RawValue/JCS entry point when provenance matters.
- `JsonTreeReader` operates on an existing `serde_json::Value`. It measures an existing Number by the byte length of its current compact display representation. It does not convert the number to `f64` or impose the ordinary text encoder range. With `arbitrary_precision`, a stored `1e400` can be accounted without panic, and a 39-digit integer counts as 39 bytes. Ordinary `JsonEncoder` and `JcsEncoder` still reject numbers outside their own contracts.

The crate does not enable serde_json's `arbitrary_precision` feature itself. If another dependency unifies that feature, the encoder recognizes serde_json's exact private Number struct protocol: one string payload delivered by `serialize_str` or one `collect_str`. It validates the payload with the shared JSON number scanner before writing or constructing a Value. A normal object with a key named `$serde_json::private::Number` remains an object. The separate `RawValue` private protocol is also supported and validated. Malformed private protocol shapes return `JsonSerializationErrorKind::InvalidSerializerState` with `JsonSerializerStateError::InvalidNumberProtocol` or `InvalidRawValueProtocol`; diagnostic text does not expose the payload.

JCS remains independent: raw text uses finite binary64 for every number and rejects overflow such as `1e400`; an already materialized wide `Number` under `arbitrary_precision` does not make `JcsEncoder` accept it.

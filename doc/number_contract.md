# JSON number contract

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

## Exact and wider values

Use a JSON string or an explicit domain representation for integers below
`i64::MIN` or above `u64::MAX`. Use a decimal string or a coefficient/scale
object for money and other exact decimals. A bare fractional JSON number has
binary64 semantics and is not an exact decimal wire format.

## Serde boundaries

`qubit-json` does not enable serde_json's `arbitrary_precision` feature and
does not interpret its former private Number marker. An object whose key is
`$serde_json::private::Number` is an ordinary object and is budgeted as such.
The independent `RawValue` integration remains supported.

`JsonValueSeed` sees already-decoded Serde events. It can reject wide numeric
events that do not fit the value model, but it cannot inspect the original
number token or enforce lexical budgets. Route JSON text through `JsonDecoder`
when syntax, range, and `NumberBytes` guarantees are required.

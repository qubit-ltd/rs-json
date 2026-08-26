# Qubit JSON

[![Rust CI](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-json/coverage-badge.json)](https://qubit-ltd.github.io/rs-json/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-json.svg?color=blue)](https://crates.io/crates/qubit-json)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Resource-aware JSON infrastructure for Rust. It preserves Serde's data model
while making input normalization, strict codecs, decoded values, and tree work
explicitly budgeted.

## Installation

```toml
[dependencies]
qubit-json = "0.8"
qubit-budget = { version = "0.3", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

For the complete API path, see the [English user guide](doc/user_guide.md), or
the [中文用户手册](doc/user_guide.zh_CN.md). The normative numeric rules are in
the [number contract](doc/number_contract.md).

## Bounded boundary in five minutes

At an HTTP or configuration boundary, create a decoder with finite limits and
admit the document before handing the resulting value to application code:

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

The guides continue this scenario with normalization, encoding, tree
processing, diagnostics, and troubleshooting: [English](doc/user_guide.md) ·
[中文](doc/user_guide.zh_CN.md).

## Choose a domain

| Domain | Use it for | Boundary |
| --- | --- | --- |
| `decode` | Normalizing text inputs and strict JSON bytes | Never guesses missing quotes, commas, or braces |
| `encode` | Strict JSON output | Stateful encoder objects with caller-owned sessions |
| `value` | Constructing `serde_json::Value` from Serde events | Charges the decoded value transaction |
| `value::traverse` | Iterative reads or mutations of materialized values | Mutable processing is incremental, not transactional |

`qubit-budget` owns limits, resource identities, budgets, and sessions.
`qubit-json` owns JSON normalization, lexical validation, text codecs, value
construction, and traversal.

## Lenient input

`NormalizingJsonDecoder` is a reusable object with immutable
`NormalizingJsonDecodePolicy`. It can remove only configured noise, then
deserialize directly into the requested type.

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_json::decode::{NormalizingJsonDecodePolicy, NormalizingJsonDecoder};

let mut decoder = NormalizingJsonDecoder::owned(
    NormalizingJsonDecodePolicy::builder().build(),
    JsonDecodeLimits::builder()
        .max_input_bytes(1024)
        .build(),
);
let value = decoder.decode_value("```json\n{\"ok\":true}\n```")?;
assert_eq!(value["ok"], true);
# Ok::<(), qubit_json::decode::JsonDecodeError>(())
```

For cumulative accounting, construct a stateful decoder with
`NormalizingJsonDecoder::new`. Raw input and normalized input charges
remain after an attempt; decoded-value charges commit only after the complete
typed decode succeeds. Errors are redacted by default. Enable
`DiagnosticPolicy::Detailed` only where input-derived diagnostics are safe.
Normalization policy never carries resource limits: pass `JsonDecodeLimits`
to `owned`, or a `JsonDecodeSession` to `new`. Explicitly pass
`JsonDecodeLimits::default()` only when unlimited decoding is intended.

When normalized text must be decoded more than once, borrowed by the result,
or materialized through a Serde seed, call `prepare_str`/`prepare_utf8` once and
then decode the returned `NormalizedJsonDocument`. Preparation commits raw and
normalized input charges once; every successful document decode commits its
own value charges. Unescaped JSON strings may borrow from the document, while
escaped strings require an owned target because Serde must materialize the
unescaped value.

## Strict text objects

Strict APIs do not repair text. Construct a decoder or encoder around the
caller-owned session and use its methods for one or more documents. Codecs do
not implement `Default`; call `owned(limits)` normally, or `unlimited()` only
when an unbounded standard session is intentional.

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonResource};
use qubit_json::decode::JsonDecoder;

let mut decoder = JsonDecoder::owned(
    JsonDecodeLimits::<JsonResource, usize>::new(),
);
let value: serde_json::Value = decoder.decode_utf8(br#"{"ok":true}"#)?;
assert_eq!(value["ok"], true);
# Ok::<(), qubit_json::decode::JsonDecodeError<
#     qubit_budget::json::JsonResource,
# >>(())
```

```rust
use qubit_budget::json::{JsonEncodeLimits, JsonResource};
use qubit_json::encode::JsonEncoder;

let value = serde_json::json!({"ok": true});
let mut encoder = JsonEncoder::owned(
    JsonEncodeLimits::<JsonResource, usize>::new(),
);
let bytes = encoder.to_vec(&value)?;
assert_eq!(bytes, br#"{"ok":true}"#);
# Ok::<(), qubit_json::encode::JsonEncodeError<
#     qubit_budget::json::JsonResource,
# >>(())
```

`JsonEncoder::write_buffered` commits only after complete output is ready
for its writer. `write_incremental` retains accepted output prefixes when a
streaming write fails.

## Number contract and browser interoperability

Strict decoding and encoding support negative integers through `i64::MIN`,
non-negative integers through `u64::MAX`, and fractional or exponential JSON
numbers that are finite `f64` values. This is deliberately wider than
JavaScript's safe-integer range (`2^53 - 1`) so Java `long` identifiers can
remain numeric on the wire. Browser clients must use a parser that preserves
these integers and maps them to `BigInt` where necessary. The JavaScript `n`
suffix is source-code syntax and is never valid JSON.

Integers below `i64::MIN` or above `u64::MAX`, and exact decimal values that
must not undergo binary floating-point rounding, need a string or explicit
domain representation. `NumberBytes` is an independent resource limit on the
original token; it does not change the representable range. This crate does
not enable serde_json's arbitrary-precision mode and treats its former private
number-marker key as an ordinary object key.

## Errors and budget semantics

The public decoding error model is shared by both decoder facades:

1. `decode::JsonDecodeError` for strict and normalizing decode failures; inspect
   `kind()` and `stage()` instead of matching private implementation details.
2. `encode::JsonEncodeError` for strict budget, raw JSON, serialization, or I/O failure.
3. `decode::JsonSyntaxError` for stable syntax reason and location metadata.
4. `value::traverse::JsonTreeProcessError` for traversal budget or visitor failure.
5. `value::traverse::JsonTreeMutateError` for input-budget, visitor, or
   output-budget failure during in-place mutation.

Budgeted operations use transactions: staged decoded-value or output charges
commit on their documented success boundary. Input charges intentionally remain
visible in decode sessions after failed attempts.

## Advanced values and trees

`strict` describes JSON syntax, number range, and resource admission; it does
not imply unique object keys. Duplicate-key behavior comes from the requested
Serde target: `serde_json::Value` and `serde_json::Map` keep the last value,
while some typed structs reject repeated fields. To require unique keys at
every object depth, decode `DuplicateKeyRejectingJsonValue` through
`JsonDecoder` (or `NormalizingJsonDecoder` when normalization is intentional).
The [user guide](doc/user_guide.md#duplicate-object-keys) contains a complete
composition example.

`JsonValueSeed` builds a materialized value while charging a caller transaction.
Because a seed sees decoded Serde events rather than the original token, it
cannot enforce text lexeme or numeric-range rules; route JSON text through
`JsonDecoder` for those guarantees.
`JsonTreeReader` visits every admitted node without Rust recursion. Its
`account` method stages whole-tree charges in the caller's existing transaction
without invoking a visitor or committing it;
`JsonTreeMutator` first admits the original tree, applies in-place visitor
changes, and then admits the complete mutated tree. It returns
`JsonTreeMutateError::InputBudget`, `::Visitor`, or `::OutputBudget`; visitor
and output failures retain mutations already made. A visitor can return
`JsonTreeControl::SkipSubtree` to skip descendant callbacks, while final output
accounting still covers every resulting descendant. `JsonTreeBudgetTracker` is
the convenient reusable choice for whole-tree accounting.

These facilities account JSON resource limits, not application-specific
semantics. Choose limits suitable for your trust boundary and keep detailed
diagnostics out of untrusted logs.

## Testing

```bash
# Run tests with the default feature set
cargo test

# Run tests with all declared features
cargo test --all-features

# Project CI checks
./ci-check.sh

# Check code coverage
./coverage.sh
```

## License

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.

## Contributing

Contributions are welcome. Please follow the Rust API guidelines, keep public
API documentation and tests current, and run `./align-ci.sh` to format code and
`./ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [https://github.com/qubit-ltd/rs-json](https://github.com/qubit-ltd/rs-json)

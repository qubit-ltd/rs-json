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
# Ok::<(), qubit_json::decode::NormalizingJsonDecodeError>(())
```

For cumulative accounting, construct a stateful decoder with
`NormalizingJsonDecoder::from_session`. Raw input and normalized input charges
remain after an attempt; decoded-value charges commit only after the complete
typed decode succeeds. Errors are redacted by default. Enable
`DiagnosticPolicy::Detailed` only where input-derived diagnostics are safe.
Normalization policy never carries resource limits: pass `JsonDecodeLimits`
to `owned`, or a `JsonDecodeSession` to `from_session`. Explicitly pass
`JsonDecodeLimits::default()` only when unlimited decoding is intended.

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

## Errors and budget semantics

The five public error types are domain-owned:

1. `decode::NormalizingJsonDecodeError` for normalization and lenient typed decode.
2. `decode::JsonDecodeError` for strict budget, syntax, or typed decode failure.
3. `encode::JsonEncodeError` for strict budget, raw JSON, serialization, or I/O failure.
4. `decode::JsonSyntaxError` for stable syntax reason and location metadata.
5. `value::traverse::JsonTreeProcessError` for traversal budget or visitor failure.

Budgeted operations use transactions: staged decoded-value or output charges
commit on their documented success boundary. Input charges intentionally remain
visible in decode sessions after failed attempts.

## Advanced values and trees

`JsonValueSeed` builds a materialized value while charging a caller transaction.
`JsonTreeReader` visits every admitted node without Rust recursion. Its
`account` method stages whole-tree charges in the caller's existing transaction
without invoking a visitor or committing it;
`JsonTreeMutator` applies in-place visitor changes and can skip a rejected
subtree through `JsonTreeBudgetRejection`. `JsonTreeBudgetTracker` is the
convenient reusable choice for whole-tree accounting.

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

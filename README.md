# Qubit JSON

[![Rust CI](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-json/coverage-badge.json)](https://qubit-ltd.github.io/rs-json/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-json.svg?color=blue)](https://crates.io/crates/qubit-json)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Resource-aware JSON infrastructure for Rust services, configuration readers,
and data pipelines. It complements `serde_json` by admitting JSON under
caller-chosen limits before untrusted input consumes unbounded parsing,
materialization, or output resources, while preserving Serde's data model.

## Installation

```toml
[dependencies]
qubit-json = "0.8"
qubit-budget = { version = "0.3", features = ["json"] }
serde_json = "1.0"
```

Add `serde = { version = "1.0", features = ["derive"] }` when decoding into
derived application types.

## Quick start: admit an HTTP request body

This example accepts one request body containing a full-range `u64` identifier,
then demonstrates that an oversized body is rejected before a decoded value is
committed:

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
    let value: serde_json::Value =
        decoder.decode_utf8(br#"{"id":18446744073709551615,"ok":true}"#)?;
    assert_eq!(value["id"], serde_json::json!(u64::MAX));

    let small_limits = limits.into_builder().max_input_bytes(8).build();
    let mut small_decoder = JsonDecoder::with_limits(small_limits);
    let error = small_decoder
        .decode_utf8::<serde_json::Value>(br#"{"ok":true}"#)
        .expect_err("the request body must exceed eight bytes");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    Ok(())
}
```

Unlike calling `serde_json::from_slice` alone, this boundary makes resource
admission explicit and exposes a stable error category through `kind()`. Apply
schema validation, authorization, and domain rules after a value is admitted.

## Why this project exists

Valid JSON can still be too large, too deep, or too expensive to materialize.
`qubit-json` keeps JSON syntax and Serde compatibility while letting callers
bound raw and normalized input, nesting, nodes, collection sizes, keys, strings,
numbers, payload, and encoded output. It also makes cumulative accounting and
commit boundaries explicit through `qubit-budget` sessions and transactions.

## What it provides

| Domain | Use it for | Boundary |
| --- | --- | --- |
| `decode` | Strict JSON admission or explicitly configured text normalization | Normalization applies only configured transformations; it never invents missing JSON syntax |
| `encode` | Budgeted strict JSON output | Buffered accounting commits after a complete write, although an I/O failure can still leave bytes in the external writer |
| `value` | Building budgeted `serde_json::Value` trees from Serde events | A seed cannot inspect original number text or enforce text-level range rules |
| `value::traverse` | Iterative reads or in-place mutations of materialized values | Mutation is incremental; visitor and output-budget failures do not roll back prior changes |

`qubit-budget` owns limits, resource identities, budgets, and sessions.
`qubit-json` owns JSON normalization, lexical validation, text codecs, value
construction, and traversal.

## Explicit boundaries

- Strict admission validates JSON syntax and the documented numeric range; it
  does not require object keys to be unique. Choose a target such as
  `DuplicateKeyRejectingJsonValue` when uniqueness is part of the contract.
- Negative integers fit `i64`, non-negative integers fit `u64`, and fractional
  or exponential values must be finite `f64`. Use strings or domain types for
  wider integers or exact decimals that must avoid binary rounding.
- Set finite limits at every untrusted boundary. Unlimited sessions are an
  explicit opt-out, not a safe default for request handling.
- Diagnostics are redacted by default. Enable `DiagnosticPolicy::Detailed`
  only where input-derived details are safe to retain and log.

## Learn more

- [English user guide](doc/user_guide.md) ·
  [中文用户手册](doc/user_guide.zh_CN.md)
- [JSON number contract](doc/number_contract.md) ·
  [JSON 数字契约](doc/number_contract.zh_CN.md)
- [API documentation](https://docs.rs/qubit-json/0.8.0/qubit_json/)

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

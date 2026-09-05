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
Use `JsonDecoder` when the input must already be strict JSON, or
`NormalizingJsonDecoder` when the boundary explicitly permits controlled
cleanup of JSON embedded in external text.

## Installation

Current `0.9` release:

```toml
[dependencies]
qubit-json = "0.9"
qubit-budget = { version = "0.5", features = ["json"] }
serde_json = "1.0"
```

Local checkout:

```toml
[dependencies]
qubit-json = { version = "0.9", path = "../rs-json" }
qubit-budget = { version = "0.5", features = ["json"] }
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

## Choose the decoding boundary

Choose the entry point from the input contract, not from whether the payload
happens to parse:

| Input contract | API |
| --- | --- |
| Input must already be complete, strict JSON | `JsonDecoder` |
| Specific presentation artifacts are allowed before strict decoding | `NormalizingJsonDecoder` with an explicit policy |
| Normalized text must be inspected, decoded repeatedly, or borrowed by the result | `NormalizingJsonDecoder::prepare_str` / `prepare_utf8`, followed by `NormalizedJsonDocument` decoding methods |

The default normalization policy can trim surrounding whitespace, remove one BOM,
unwrap one outer JSON Markdown fence, and escape raw ASCII control characters
inside strings. It is controlled normalization, not another JSON dialect:
comments, trailing commas, unquoted keys, and missing syntax remain errors.

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
| `encode` | Budgeted strict JSON output | Value accounting commits after complete serialization; an I/O failure can still leave bytes in an external writer |
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
- Set finite limits at every untrusted boundary. Use `unlimited()` only for
  trusted input or data already admitted by another layer; an outer output
  bound cannot replace input and structural admission before parsing.
- The strict and normalizing decoders consume complete `&str` or `&[u8]`
  inputs. Their input-byte limit admits the supplied slice; it does not cap
  memory already allocated by an HTTP body aggregator or another transport
  layer. Apply a bounded read or body-aggregation limit before decoding.
- Diagnostics are redacted by default. Configure a strict decoder with
  `JsonDecoder::with_diagnostic_policy`, or a normalizing decoder through its
  normalization policy. Redacted errors retain only stable classifications and
  source coordinates: they never retain an offending byte, token/key/value
  text, or parser/Serde source. Enable `DiagnosticPolicy::Detailed` only
  where input-derived sources are safe to retain and log.

## Learn more

- [English user guide](doc/user_guide.md) ·
  [中文用户手册](doc/user_guide.zh_CN.md)
- [JSON number contract](doc/number_contract.md) ·
  [JSON 数字契约](doc/number_contract.zh_CN.md)
- [Design documents](doc/json_design.md) ·
  [设计文档](doc/json_design.zh_CN.md)
- [Benchmark baseline](doc/benchmark_baseline.md) ·
  [基准测试基线](doc/benchmark_baseline.zh_CN.md)
- [Migration from 0.3 to 0.8](doc/migration_0_3_to_0_8.md) ·
  [从 0.3 迁移到 0.8](doc/migration_0_3_to_0_8.zh_CN.md)
- [Changelog](CHANGELOG.md) · [中文变更记录](CHANGELOG.zh_CN.md)
- [Released API documentation](https://docs.rs/qubit-json); for the current
  branch API, run `cargo doc --all-features --open`

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

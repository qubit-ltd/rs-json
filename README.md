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

```toml
[dependencies]
qubit-json = "0.8"
qubit-budget = { version = "0.4", features = ["json"] }
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

## Normalize JSON from external text

`NormalizingJsonDecoder` is the main entry point when the input is intended to
be JSON but may contain explicitly permitted transport or presentation
artifacts. Typical sources include generated text, copied Markdown snippets,
and text configuration files. It first applies a
`NormalizingJsonDecodePolicy`, then runs the same strict JSON syntax, numeric,
and resource admission used by `JsonDecoder`.

`NormalizingJsonDecodePolicy::lenient()` enables the library's standard
normalization profile:

- trim surrounding whitespace;
- remove one leading UTF-8 BOM;
- unwrap one outer JSON Markdown code fence, with an optional closing fence;
- escape raw ASCII control characters found inside JSON strings; and
- redact input-derived diagnostic details.

This is controlled normalization, not a permissive JSON dialect. It does not
accept comments, trailing commas, unquoted keys, or missing JSON syntax.

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;

let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
    .max_input_bytes(4096)
    .max_normalized_input_bytes(4096)
    .max_depth(32)
    .max_nodes(256)
    .max_sequence_items(64)
    .max_map_entries(64)
    .max_key_bytes(128)
    .max_string_bytes(2048)
    .max_number_bytes(20)
    .max_payload_bytes(4096)
    .build();
let mut decoder = NormalizingJsonDecoder::with_limits(
    NormalizingJsonDecodePolicy::lenient(),
    limits,
);
let value = decoder
    .decode_object_str::<serde_json::Value>("```json\n{\"ok\":true}\n```")
    .expect("the configured policy accepts a JSON Markdown fence");
assert_eq!(value["ok"], true);
```

Choose the decoding entry point according to the input contract:

| Input contract | API |
| --- | --- |
| Input must already be complete, strict JSON | `JsonDecoder` |
| Specific presentation artifacts are allowed before strict decoding | `NormalizingJsonDecoder` with an explicit policy |
| Normalized text must be inspected, decoded repeatedly, or borrowed by the result | `NormalizingJsonDecoder::prepare_str` / `prepare_utf8`, followed by `NormalizedJsonDocument` decoding methods |

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

## Core API at a glance

| API | Purpose |
| --- | --- |
| `decode::JsonDecoder` | Strictly validates and decodes complete JSON strings or UTF-8 byte slices, with optional top-level object/array checks and reusable cumulative accounting |
| `decode::NormalizingJsonDecoder` | Normalizes explicitly permitted external-text artifacts, then performs the same strict decoding and resource admission as `JsonDecoder` |
| `decode::NormalizingJsonDecodePolicy` / `NormalizingJsonDecodePolicyBuilder` | Selects each permitted normalization and whether diagnostics are redacted or detailed; resource limits remain separate |
| `decode::NormalizedJsonDocument` | Retains normalized text for inspection, borrowed deserialization, or repeated decoding without charging the input a second time |
| `decode::JsonDecodeError` and diagnostic enums | Exposes stable error kind, processing stage, root expectation, and syntax reason without requiring callers to parse messages |
| `encode::JsonEncoder` | Serializes strict compact JSON to a byte vector or writer while enforcing output and encoded-value limits |
| `value::JsonValueEncoder` | Projects any `Serialize` value into a strict `serde_json::Value`; failures expose a broad category, a precise reason, and privacy-safe typed details |
| `value::AccountingJsonValueSeed` | Builds a `serde_json::Value` from any Serde deserializer while staging decoded-value charges in a caller-owned transaction |
| `value::DuplicateKeyRejectingJsonValue` / `DuplicateKeyRejectingJsonValueSeed` | Materializes JSON while rejecting duplicate object keys recursively |
| `value::traverse::JsonTreeBudgetTracker` | Accounts a complete materialized tree with a reusable, internally owned value budget |
| `value::traverse::JsonTreeReader` / `JsonTreeVisitor` | Performs non-recursive, budget-aware, read-only traversal with node depth and location context |
| `value::traverse::JsonTreeMutator` / `JsonTreeMutVisitor` | Performs non-recursive in-place mutation between separate input and output transactions; `JsonTreeControl` selects whether callbacks descend into children |

Encode with output and value limits:

```rust
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncoder;

let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
    .max_output_bytes(64)
    .max_depth(4)
    .max_nodes(8)
    .build();
let mut encoder = JsonEncoder::with_limits(limits);
let bytes = encoder
    .to_vec(&serde_json::json!({"ok": true}))
    .expect("the value fits the configured limits");
assert_eq!(bytes, br#"{"ok":true}"#);
```

Handle materialized-value encoding failures without parsing display text:

```rust
use qubit_json::value::JsonIntegerSignedness;
use qubit_json::value::JsonValueEncodeErrorCategory;
use qubit_json::value::JsonValueEncodeErrorKind;
use qubit_json::value::JsonValueEncoder;

let error = JsonValueEncoder::new()
    .encode(&u128::MAX)
    .expect_err("u128::MAX is outside the strict JSON integer range");
assert_eq!(error.category(), JsonValueEncodeErrorCategory::Number);
assert_eq!(
    error.kind(),
    JsonValueEncodeErrorKind::IntegerOutOfRange {
        signedness: JsonIntegerSignedness::Unsigned,
    },
);
assert!(error.is_number_error());
```

Reject ambiguous objects and account an already materialized tree:

```rust
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::DuplicateKeyRejectingJsonValue;
use qubit_json::value::traverse::JsonTreeBudgetTracker;

let duplicate = serde_json::from_str::<DuplicateKeyRejectingJsonValue>(
    r#"{"role":"reader","role":"admin"}"#,
);
assert!(duplicate.is_err());

let mut tracker = JsonTreeBudgetTracker::new(
    JsonValueLimits::<JsonResource, usize>::builder()
        .max_depth(4)
        .max_nodes(8)
        .build(),
);
tracker
    .account(&serde_json::json!({"role": "reader"}))
    .expect("the complete tree fits the configured limits");
```

## Performance model

Resource checks are paid only where they can change the result. Encoding
without an output limit serializes directly into its owned byte vector while
retaining value accounting. Tree traversal with an unlimited value transaction
skips admission work; bounded traversal keeps the same checks and error
semantics. The Criterion suites keep both sides visible:

```bash
cargo bench --bench budgeted_serde_json
cargo bench --bench tree_bench
```

`tree_bench` reports unlimited and bounded read/mutation cases separately, so
future fast-path changes can be checked against the protected path rather than
hiding its cost in one aggregate result.

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
  normalization policy. Enable `DiagnosticPolicy::Detailed` only where
  input-derived details are safe to retain and log.

## Learn more

- [English user guide](doc/user_guide.md) ·
  [中文用户手册](doc/user_guide.zh_CN.md)
- [JSON number contract](doc/number_contract.md) ·
  [JSON 数字契约](doc/number_contract.zh_CN.md)
- [Design documents](doc/json_design.md) ·
  [设计文档](doc/json_design.zh_CN.md)
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

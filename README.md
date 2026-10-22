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

For the `0.12` version described by this checkout:

```toml
[dependencies]
qubit-json = "0.12"
qubit-budget = { version = "0.9.1", features = ["json"] }
serde_json = "1.0"
```

Local checkout:

```toml
[dependencies]
qubit-json = { version = "0.12", path = "../rs-json" }
qubit-budget = { version = "0.9.1", features = ["json"] }
serde_json = "1.0"
```

Add `serde = { version = "1.0", features = ["derive"] }` when decoding into
derived application types.

To enable RFC 8785 canonical output, opt into the feature explicitly:

```toml
qubit-json = { version = "0.12", features = ["jcs"] }
```

## Quick start: decode an already aggregated, size-limited HTTP body

This example decodes bytes from an HTTP body that the transport layer has
already aggregated under a size limit. It accepts a full-range `u64`
identifier, then demonstrates that oversized JSON input is rejected before a
decoded value is committed:

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
the transport size limit before aggregating the body into the slice shown
above. The application flow is: bounded transport aggregation → JSON admission
with `JsonDecoder` → schema, authorization, and domain validation.

`qubit-json` does not provide HTTP body aggregation. For a downstream HTTP
integration that applies the transport limit before `BoundedJson` performs JSON
admission, see the [`rs-web` controller example](https://github.com/qubit-ltd/rs-web/blob/main/examples/controller_crud.rs).
This is a changeable downstream reference; the [user guide](doc/user_guide.md#advanced-usage)
explains `decode_input`.

## Choose the decoding boundary

Choose the entry point from the input contract, not from whether the payload
happens to parse:

| Input contract | API |
| --- | --- |
| Input must already be complete, strict JSON | `JsonDecoder` |
| A byte-oriented source should be read through EOF into an owned result | `JsonDecoder::decode_input` |
| Specific presentation artifacts are allowed before strict decoding | `NormalizingJsonDecoder` with an explicit policy |
| Normalized text must be inspected, decoded repeatedly, or borrowed by the result | `NormalizingJsonDecoder::prepare_str` / `prepare_utf8`, followed by `NormalizingJsonDecoder::decode_precharged_document`, `decode_precharged_object_document`, or `decode_precharged_array_document` |

The default normalization policy can trim surrounding whitespace, remove one BOM,
unwrap one outer JSON Markdown fence, and escape raw ASCII control characters
inside strings. It is controlled normalization, not another JSON dialect:
comments, trailing commas, unquoted keys, and missing syntax remain errors.

## Canonical bytes for a signed payload

When an application needs stable bytes for a hash or signature, enable `jcs`
and pass an already constructed `serde_json::Value` to `JcsEncoder`:

```rust
use qubit_budget::json::{JsonEncodeLimits, JsonResource};
use qubit_json::encode::JcsEncoder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(4096)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 16 * 1024);
    let value = serde_json::json!({"b": 2, "a": 1.0});
    assert_eq!(encoder.to_vec(&value)?, br#"{"a":1,"b":2}"#);
    Ok(())
}
```

The `jcs` feature is disabled by default. `JcsEncoder` canonicalizes the
`Value` model; it does not accept arbitrary `Serialize` values or raw JSON
text. With `JcsDecoder`, raw input duplicate keys are rejected before a
`Value` is built; once duplicates are collapsed into a map, they cannot be
recovered. JCS numbers use IEEE 754 binary64 semantics, so
`9007199254740993` can be emitted as `9007199254740992`; encode an integer as a
JSON string when its exact decimal digits must be preserved. The regular
`JsonDecoder` does not accept every JCS number lexeme as raw text.

For raw JSON text with JCS number semantics, enable `jcs` and use
`JcsDecoder`. It returns an owned `serde_json::Value`, rounds every number to
finite binary64, and rejects keys that are duplicates after JSON unescaping.
See the [user guide](doc/user_guide.md#decode-raw-text-with-jcs-semantics) for
finite-budget usage and the rounding, Unicode, depth, and input-buffering
boundaries. `JcsDecoder` parses input; it does not verify that the input bytes
are already canonical. `JcsEncoder` still produces canonical output.

## Version 0.12 migration

Version 0.12 adds strict handling for the private Serde protocols used by
`serde_json::RawValue` and `serde_json::Number`. See the [migration guide](doc/migration_0_12.md)
for the new `InvalidNumberProtocol` state error, the budget type identity change,
and downstream version updates.

## Why this project exists

Valid JSON can still be too large, too deep, or too expensive to materialize.
`qubit-json` keeps JSON syntax and Serde compatibility while letting callers
bound raw and normalized input, nesting, nodes, collection sizes, keys, strings,
numbers, payload, and encoded output. It also makes cumulative accounting and
commit boundaries explicit through `qubit-budget` sessions and transactions.

## What it provides

| Domain | Use it for | Boundary |
| --- | --- | --- |
| `decode` | Strict JSON admission, explicitly configured normalization, or JCS text decoding with `JcsDecoder` | Ordinary decoding keeps its integer range; JCS decoding uses binary64 and rejects duplicate keys |
| `encode` | Budgeted strict JSON output | Value accounting commits after complete serialization; an I/O failure can still leave bytes in an external writer |
| `value` | Building budgeted `serde_json::Value` trees from Serde events | A seed cannot inspect original number text or enforce text-level range rules |
| `value::traverse` | Iterative reads or in-place mutations of materialized values | Mutation is incremental; visitor and output-budget failures do not roll back prior changes |

`qubit-budget` owns limits, resource identities, budgets, and sessions.
`qubit-json` owns JSON normalization, lexical validation, text codecs, value
construction, and traversal.

For JCS, `Budget` identifies value or output limits, while `WorkingMemory`
identifies the separate auxiliary-capacity limit configured with
`JcsEncoder::with_limits(limits, max_working_bytes)`.

## Explicit boundaries

- Ordinary strict admission validates JSON syntax and the documented numeric
  range; it does not require object keys to be unique. Choose a target such as
  `DuplicateKeyRejectingJsonValue`, or use `JcsDecoder` for raw JCS text, when
  uniqueness is part of the contract.
- Ordinary text decoding and encoding require negative integers to fit `i64` and
  non-negative integers to fit `u64`; `JcsDecoder` rounds every number to a
  finite `f64`. Thus `9007199254740993` becomes `9007199254740992`. Use strings
  or domain types when every decimal digit must be preserved.
- `JsonValueEncoder` recognizes only the exact private `Number` Serde struct protocol; an ordinary map with the same field name remains an object. Malformed `RawValue` and `Number` protocols return `JsonSerializationErrorKind::InvalidSerializerState`, with `JsonSerializerStateError::InvalidRawValueProtocol` or `InvalidNumberProtocol` respectively. Seed-based value construction only sees native numeric events and cannot infer private map provenance.
- Tree traversal counts an existing `serde_json::Number` using the length of its current compact display. Under `arbitrary_precision`, this permits accounting for a stored wide number such as `1e400`; ordinary and JCS encoders still reject numbers outside their own finite range.
- Set finite limits at every untrusted boundary. `unlimited()` disables value,
  output, and working-memory limits; use it only for trusted input or data
  already admitted by another layer. An outer output
  bound cannot replace input and structural admission before parsing.
- Slice-based strict and normalizing decoder methods consume complete `&str` or
  `&[u8]` inputs; use them when the decoded result must borrow from the input.
  `JsonDecoder::decode_input` instead reads an `Input<Item = u8>` source through
  EOF and returns an owned `DeserializeOwned` value. It buffers the complete
  document; without `max_input_bytes`, that temporary memory use has no
  configured limit. Apply a bounded read or body-aggregation limit before
  decoding. This strict JSON method is not a raw-text JCS canonicalization
  entry point.
- `JcsDecoder::decode_input` also reads and buffers the complete input. With a
  finite input limit, it may consume one uncharged probe byte that cannot be
  returned to the source. Lexical budgets are not exact heap-memory bounds:
  result-container and input-buffer capacities and allocator metadata are not
  measured precisely. JCS Value construction separately caps nested
  containers at 127, including with unlimited budgets; the 128th container
  returns `MaterializationDepthLimit`. This differs from root-inclusive
  `max_depth`. Duplicate keys are compared after JSON unescaping, without
  Unicode normalization; canonical-byte verification is outside this API.
- `JcsEncoder::write_buffered` and `write_incremental` can leave and charge
  bytes accepted by an external destination before a later write or flush
  failure. Incremental mode writes directly and flushes on success; value
  accounting is committed only after the complete operation succeeds.
  `max_working_bytes` bounds auxiliary traversal and object-sorting capacity,
  not the input `Value`, allocator metadata, or output buffer capacity.
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
- [0.12 migration guide](doc/migration_0_12.md) · [迁移指南](doc/migration_0_12.zh_CN.md)
- [Benchmark guide](doc/benchmark_baseline.md) ·
  [基准测试指南](doc/benchmark_baseline.zh_CN.md)
- [Released API documentation](https://docs.rs/qubit-json); for the current
  branch API, run `cargo doc --all-features --open`

## Testing

```bash
# Run tests with the default feature set
cargo test

# Run tests with all declared features
cargo test --all-features

# Run the standalone Markdown documentation examples
cargo test --manifest-path tests/fixtures/documentation_examples/Cargo.toml --doc --locked

# Project CI checks
./.infra/bin/ci-check.sh

# Check code coverage
./.infra/bin/coverage.sh
```

## License

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.

## Contributing

Contributions are welcome. Please follow the Rust API guidelines, keep public
API documentation and tests current, and run `./.infra/bin/align-ci.sh` to format code and
`./.infra/bin/ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [https://github.com/qubit-ltd/rs-json](https://github.com/qubit-ltd/rs-json)

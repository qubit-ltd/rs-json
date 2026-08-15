# Qubit JSON

[![Rust CI](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-json/coverage-badge.json)](https://qubit-ltd.github.io/rs-json/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-json.svg?color=blue)](https://crates.io/crates/qubit-json)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Resource-aware JSON infrastructure for Rust. It combines predictable lenient
input normalization, strict text codecs, budgeted value construction, and
non-recursive tree processing without hiding Serde behind a new data model.

## Choose the boundary you need

| Module | Use it for | Boundary |
| --- | --- | --- |
| `lenient` | Normalizing fenced or lightly noisy text, then deserializing `T` | Only documented repairs; no guessed quotes, commas, or braces |
| `text` | Strict budget-aware JSON decode and encode | Caller-owned decode/encode sessions; no text repair |
| `value` | Building a `serde_json::Value` through a Serde seed | Decoded-value accounting; the implementation stays private |
| `tree` | Visiting or mutating a materialized `Value` iteratively | `process_mut` is non-transactional |

`qubit-budget` owns JSON resource identities, limits, budgets, and mutable
sessions. `qubit-json` owns normalization, lexical admission, strict text
adapters, value construction, and tree traversal.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
qubit-json = "0.7"
qubit-budget = { version = "0.4", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
```

The direct `serde` dependency is only needed when deriving `Deserialize`.
If your code names `serde_json::Value` or uses `serde_json` macros, add
`serde_json` as a direct dependency. This crate intentionally does not
re-export it.

## Quick Start

### Decode a fenced response with cumulative budgets

Suppose a service receives Markdown-wrapped JSON from a text channel, needs a
typed result, and must account for all work across retries. Reuse one
`JsonDecodeSession`: raw input, normalized input, and decoded-value resources
are charged in that order. The normalized document is lexically admitted, then
deserialized directly into `T`; there is no intermediate `serde_json::Value`.

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonDecodeSession};
use qubit_json::lenient::LenientJsonDecoder;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Reply {
    ok: bool,
}

let limits = JsonDecodeLimits::empty()
    .with_max_input_bytes(64)
    .with_max_normalized_input_bytes(32)
    .with_max_nodes(2)
    .with_max_map_entries(1)
    .with_max_key_bytes(2)
    .with_max_payload_bytes(2);
let mut session = JsonDecodeSession::owned(limits);
let decoder = LenientJsonDecoder::default();

let reply: Reply = decoder.decode_with_session(
    "```json\n{\"ok\":true}\n```",
    &mut session,
)?;
assert_eq!(reply, Reply { ok: true });
assert_eq!(session.value_budget().used_nodes(), Some(2));
# Ok::<(), qubit_json::lenient::LenientJsonDecodeError>(())
```

Input charges remain in the caller-owned session even if a later budget check,
syntax check, or target deserialization fails; value accounting is committed
only after the complete top-level value succeeds. A `Budget`/`Admission` error exposes the structured rejection through
`measured_budget_error()`. Ordinary `decode()` remains the faster normalization
and direct-deserialization path and does not run value preflight.

## What It Provides

### Lenient decoding

- Reusable decoder object that holds immutable decoding options
- `decode<T>()`: decodes any JSON top-level value into `T`
- `decode_with_session<T>()`: adds cumulative raw, normalized, and value
  accounting before direct deserialization
- `decode_slice<T>()`: validates UTF-8 bytes and decodes them into `T`
- `decode_value()`: decodes into `serde_json::Value`
- `decode_object<T>()`: requires a top-level JSON object and deserializes `T`
  directly from normalized text
- `decode_array<T>()`: requires a top-level JSON array and deserializes its
  elements directly from normalized text

### `JsonDecodeOptions`

- Immutable presets, getters, and value-style builders for every option
- Presets: `lenient()` and `strict()`; strict mode disables text rewriting but
  retains empty-input classification, optional size limits, privacy handling,
  and stable error mapping
- `trim_whitespace`: trims leading and trailing whitespace
- `strip_utf8_bom`: strips a leading UTF-8 BOM
- `markdown_fence_policy`: selects disabled, any-language, or JSON-only fence
  stripping, together with an optional or required closing fence
- The default accepts only empty, `json`, and `jsonc` fence labels.
  Any-language stripping requires an explicit `MarkdownFencePolicy::Any`.
- `jsonc` is accepted only as a Markdown fence label; fenced content is still
  parsed as standard JSON, so comments and trailing commas remain invalid
- `escape_control_chars_in_strings`: escapes ASCII control characters inside
  JSON string literals
- `max_input_bytes`: optional byte-size limit applied before normalization
- `max_normalized_bytes`: optional byte-size limit applied to normalized JSON
  before control-character repair allocates text
- `error_privacy_policy`: selects safe redacted errors (the default) or
  explicitly requested detailed serde diagnostics

### Strict text, value, and tree infrastructure

- `text::decode_slice` / `text::decode_admitted_slice_seed` strictly decode bytes with a
  `JsonDecodeSession`; `text::encode_to_vec` / `text::encode_to_writer` encode
  with a `JsonEncodeSession`.
- `text::JsonEncodeError::InvalidRawJson` retains the stable
  `JsonSyntaxError` reason, offset, line, and column rather than rebuilding a
  `serde_json::Error` from text.
- `value::AccountingJsonValueSeed` is the only public path for incrementally
  building `serde_json::Value` while charging a value budget.
- `tree::JsonTreeReader` accepts values whose borrow is shorter than the
  budget borrow. `process_mut` keeps mutations and budget consumption completed
  before an error; its restoration guard only keeps the root structurally
  valid and does not restore the original value.

### Explicit error model

- `Budget`: caller-owned decoded-value limits reject work during `Admission`
- `InputTooLarge`: raw or normalized input size exceeds its configured limit
- `EmptyInput`: input becomes empty after normalization
- `InvalidUtf8`: raw byte input is not valid UTF-8
- `InvalidJson`: normalized text is not valid JSON syntax
- `UnexpectedTopLevel`: top-level JSON kind does not match the requested method
- `Deserialize`: JSON is valid but cannot be deserialized into the target type
- `JsonDecodeError` exposes immutable accessors for the failure kind, stage,
  message, top-level context, raw and normalized byte sizes, and both input
  limits
- parser line and column accessors refer to normalized JSON text
- invalid UTF-8 errors expose the safe byte offset and, when known, invalid
  sequence length through `utf8_valid_up_to()` and `utf8_error_len()`
- `privacy_policy()` records the policy applied to every returned error
- under the default `Redacted` policy, parser/deserializer messages do not
  contain serde-provided input fragments and `Error::source()` is `None`
- `Detailed` preserves the complete UTF-8 or serde source and may therefore
  expose input-derived diagnostics; use it only in controlled environments

## Additional Lenient Examples

### Decode JSON Containing Raw Control Characters in Strings

```rust
use qubit_json::lenient::LenientJsonDecoder;

fn main() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("{\"text\":\"line 1\nline 2\"}")
        .expect("decoder should escape raw control characters inside strings");

    assert_eq!(value["text"], "line 1\nline 2");
}
```

### Customize Decoder Options

```rust
use qubit_json::lenient::JsonDecodeOptions;
use qubit_json::lenient::LenientJsonDecoder;

fn main() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::lenient()
            .with_max_input_bytes(Some(1024)),
    );

    let value = decoder
        .decode_value("{\"ok\":true}")
        .expect("plain JSON should still decode with custom options");

    assert_eq!(value["ok"], true);
}
```

### Set an Input Limit for Untrusted Sources

`JsonDecodeOptions::default()` deliberately leaves `max_input_bytes` and
`max_normalized_bytes` unset so the crate does not impose application-specific
limits. When inputs cross a trust boundary, configure limits appropriate to the
caller's memory and latency budget.

`max_input_bytes` applies to raw input. `max_normalized_bytes` applies after
trimming and fence removal, and is checked before control-character repair
allocates text. Escaping one raw ASCII control byte as `\\u00XX` can expand
content from one byte to six bytes.

```rust
use qubit_json::lenient::{JsonDecodeOptions, LenientJsonDecoder};

let decoder = LenientJsonDecoder::new(
    JsonDecodeOptions::default()
        .with_max_input_bytes(Some(1_048_576))
        .with_max_normalized_bytes(Some(6_291_456)),
);
let value = decoder.decode_value("{\"ok\":true}")?;

assert_eq!(value["ok"], true);
# Ok::<(), qubit_json::lenient::LenientJsonDecodeError>(())
```

### Opt In to Detailed Error Diagnostics

Detailed serde diagnostics may include values from the input. Enable them only
when the diagnostic sink and its readers are trusted.

```rust
use qubit_json::lenient::{
    ErrorPrivacyPolicy,
    JsonDecodeOptions,
    LenientJsonDecoder,
};

fn main() {
    let options = JsonDecodeOptions::default()
        .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed);
    let decoder = LenientJsonDecoder::new(options);

    let error = decoder
        .decode::<u64>(r#""not a number""#)
        .expect_err("the JSON string cannot deserialize into u64");
    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Detailed);
    assert!(std::error::Error::source(&error).is_some());
}
```

## Behavioral Contracts

### Normalization rules

When enabled, the decoder applies the following pipeline before parsing:

1. enforce the optional raw input byte-size limit
2. validate that the input is not empty
3. trim surrounding whitespace
4. strip a leading UTF-8 BOM
5. trim surrounding whitespace again
6. strip one outer backtick or tilde Markdown code fence
7. trim surrounding whitespace again
8. enforce the optional normalized JSON byte-size limit before allocation
9. escape ASCII control characters inside JSON string literals

The `lenient` module does not:

- add missing quotes
- add missing commas
- add missing braces or brackets
- rewrite arbitrary malformed JSON into guessed valid JSON

### Session and mutation failure semantics

Each `JsonDecodeSession::begin_value()` or `JsonEncodeSession::begin_value()`
creates one value attempt. Its value measurements are staged and commit only
when the complete attempt succeeds. Raw, normalized, and accepted-output byte
charges are immediate. The following matrix applies to the public APIs:

| API or failure point | Immediate accounting retained after failure | Staged value accounting | External side effects |
| --- | --- | --- | --- |
| Strict `text::decode_slice` syntax or typed failure | Raw input bytes | Rolled back | None |
| Strict `text::inspect` lexical failure | Raw input bytes | Rolled back | None |
| Lenient `decode_with_session` normalization, syntax, or typed failure | Raw and normalized input bytes | Rolled back | None |
| `text::encode_to_vec` serialization or budget failure | None before a complete buffer is accepted | Rolled back | No returned vector |
| Buffered `text::encode_to_writer` final write failure | Bytes accepted by the writer | Rolled back | The writer can retain an accepted prefix |
| Incremental `text::encode_to_writer_incremental` serialization, budget, or I/O failure | Every accepted output byte | Rolled back | The writer can retain an accepted prefix |
| Streamed value failure inside a manual session attempt | Raw, normalized, or accepted-output bytes already charged | Rolled back when the attempt drops | Any caller-managed effect remains |

Value transactions do not control external side effects: a writer, callback,
network peer, or other destination cannot be rolled back by dropping an
attempt. Reuse the same session deliberately when retained I/O charges across
attempts are part of the desired limit.

- `JsonTreeMutator::process` is incremental. Visitor mutations and
  budget charges completed before a visitor or budget error remain observable.

## When to Use

Qubit JSON is a good fit when:

- you need one resource-accounting vocabulary across text, values, and trees
- you need a reusable, configurable lenient decoder object
- your inputs are mostly valid JSON but may be wrapped or slightly noisy
- you want stable and safe-by-default error categories around `serde_json`

It is not a good fit when:

- you need aggressive repair for heavily malformed JSON
- your inputs are not actually JSON
- a plain `serde_json::from_str()` call already provides all required behavior

## Compatibility and Upgrades

Budgeted serialization recognizes private `serde_json` Number and RawValue
protocol names. Production therefore pins `serde_json` exactly to `1.0.151`,
and `src/budget/internal/serde_json_compat.rs` is the sole production owner of
those tokens. When upgrading `serde_json`:

1. update the exact version in `Cargo.toml`;
2. update both root and `fuzz/Cargo.lock` files;
3. review upstream private Number and RawValue serializers against the compat
   module;
4. run private-protocol and serializer regressions, both dependency-tree
   checks, the fuzz workspace check, and the full project quality gates.

Do not add token checks outside the compatibility module or relax the exact
version before that review succeeds.

## Learn More

- [中文文档](README.zh_CN.md)
- [设计说明（中文）](doc/json_design.zh_CN.md)
- [产品需求（中文）](doc/json_prd.zh_CN.md)
- [基准基线（中文）](doc/benchmark_baseline.zh_CN.md)
- [API documentation](https://docs.rs/qubit-json)

## Development Validation

Run the repository checks with `./align-ci.sh` followed by `./ci-check.sh`.
Criterion benchmarks include 1 KiB, 64 KiB, and 1 MiB budgeted strict and
lenient decode/encode comparisons and wide/deep tree traversal. Compile all
three benchmark targets with:

```bash
cargo bench --bench decoder_bench --no-run
cargo bench --bench budgeted_serde_json --no-run
cargo bench --bench tree_bench --no-run
```

The optional fuzz target is development tooling and is not a runtime
dependency. It exercises the default, strict, JSON-only, and required-closing
decoder policies and mutable JSON tree restoration invariants. A bounded run is scheduled by `.github/workflows/fuzz.yml`;
failures retain their reproduction artifacts. Install `cargo-fuzz` to build or
run the same target locally from the repository root:

```bash
rustup toolchain install nightly-2026-06-05 --profile minimal
cargo install cargo-fuzz --version 0.13.2 --locked
(cd fuzz && cargo +nightly-2026-06-05 fuzz build decoder)
(cd fuzz && cargo +nightly-2026-06-05 fuzz run decoder -- -max_len=4096)
(cd fuzz && cargo +nightly-2026-06-05 fuzz build json_tree_invariants)
```

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

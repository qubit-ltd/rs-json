# Qubit JSON

[![Rust CI](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-json/coverage-badge.json)](https://qubit-ltd.github.io/rs-json/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-json.svg?color=blue)](https://crates.io/crates/qubit-json)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Lenient JSON decoder for Rust, designed for non-fully-trusted text inputs.

## Overview

Qubit JSON provides a small and predictable decoding layer on top of
`serde_json`. Its core type, `LenientJsonDecoder`, normalizes a limited set of
common input issues before parsing and deserializing JSON values.

The crate is intended for cases where JSON text may come from sources such as:

- Markdown-wrapped text
- Markdown code blocks using backtick or tilde fences
- copied snippets
- CLI output streams
- other text channels that may wrap otherwise valid JSON

It is intentionally narrow. The crate does not try to be a general JSON repair
engine, and it does not attempt to guess missing quotes, commas, or braces.

## Design Goals

- **Lenient but predictable**: only handle a small set of well-defined input
  problems
- **Object-oriented API**: use a reusable `LenientJsonDecoder` instance instead
  of a loose bag of helper functions
- **Serde-first**: delegate actual parsing and deserialization to `serde_json`
- **Privacy-aware errors**: report stable, redacted diagnostics by default and
  allow detailed serde diagnostics only by explicit configuration
- **Low overhead**: avoid unnecessary allocation when normalization can borrow
  the original input

## Features

### `LenientJsonDecoder`

- Reusable decoder object that holds immutable decoding options
- `decode<T>()`: decodes any JSON top-level value into `T`
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
- `error_privacy_policy`: selects safe redacted errors (the default) or
  explicitly requested detailed serde diagnostics

### Explicit Error Model

- `InputTooLarge`: raw input size exceeds configured limit
- `EmptyInput`: input becomes empty after normalization
- `InvalidUtf8`: raw byte input is not valid UTF-8
- `InvalidJson`: normalized text is not valid JSON syntax
- `UnexpectedTopLevel`: top-level JSON kind does not match the requested method
- `Deserialize`: JSON is valid but cannot be deserialized into the target type
- `JsonDecodeError` exposes immutable accessors for the failure kind, stage,
  message, top-level context, raw and normalized byte sizes, and input limit
- parser line and column accessors refer to normalized JSON text
- `privacy_policy()` records the policy applied to every returned error
- under the default `Redacted` policy, parser/deserializer messages do not
  contain serde-provided input fragments and `Error::source()` is `None`
- `Detailed` preserves the complete UTF-8 or serde source and may therefore
  expose input-derived diagnostics; use it only in controlled environments

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
qubit-json = "0.6"
serde = { version = "1.0", features = ["derive"] }
```

The direct `serde` dependency is only needed when deriving `Deserialize` for
typed decoding, as shown in the first quick-start example below.

## Quick Start

### Decode a JSON Object from a Markdown Code Fence

```rust
use serde::Deserialize;
use qubit_json::LenientJsonDecoder;

#[derive(Debug, Deserialize)]
struct User {
    name: String,
    age: u8,
}

fn main() {
    let decoder = LenientJsonDecoder::default();
    let user: User = decoder
        .decode_object("```json\n{\"name\":\"Alice\",\"age\":30}\n```")
        .expect("decoder should extract and decode the fenced JSON object");

    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);
}
```

### Decode JSON Containing Raw Control Characters in Strings

```rust
use qubit_json::LenientJsonDecoder;

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
use qubit_json::{LenientJsonDecoder, JsonDecodeOptions};

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

`JsonDecodeOptions::default()` deliberately leaves `max_input_bytes` unset so
the crate does not impose an application-specific limit. When inputs cross a
trust boundary, configure a limit appropriate to the caller's memory and
latency budget.

The limit applies to raw input, not normalized allocation size. Escaping one
raw ASCII control byte as `\\u00XX` can expand content from one byte to six
bytes, in addition to the allocation's own overhead.

```rust
use qubit_json::{JsonDecodeOptions, LenientJsonDecoder};

let decoder = LenientJsonDecoder::new(
    JsonDecodeOptions::default().with_max_input_bytes(Some(1_048_576)),
);
let value = decoder.decode_value("{\"ok\":true}")?;

assert_eq!(value["ok"], true);
# Ok::<(), qubit_json::JsonDecodeError>(())
```

### Opt In to Detailed Error Diagnostics

Detailed serde diagnostics may include values from the input. Enable them only
when the diagnostic sink and its readers are trusted.

```rust
use qubit_json::{
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

## Normalization Rules

When enabled, the decoder applies the following pipeline before parsing:

1. enforce the optional raw input byte-size limit
2. validate that the input is not empty
3. trim surrounding whitespace
4. strip a leading UTF-8 BOM
5. trim surrounding whitespace again
6. strip one outer backtick or tilde Markdown code fence
7. trim surrounding whitespace again
8. escape ASCII control characters inside JSON string literals

The decoder does not:

- add missing quotes
- add missing commas
- add missing braces or brackets
- rewrite arbitrary malformed JSON into guessed valid JSON

## When to Use

Qubit JSON is a good fit when:

- you need a reusable, configurable JSON decoder object
- your inputs are mostly valid JSON but may be wrapped or slightly noisy
- you want stable and safe-by-default error categories around `serde_json`

It is not a good fit when:

- you need aggressive repair for heavily malformed JSON
- your inputs are not actually JSON
- a plain `serde_json::from_str()` call is already sufficient

## Alignment Notes

This README reflects the current object model:

- `LenientJsonDecoder` owns an internal `LenientJsonNormalizer`.
- Public decoding APIs are `decode`, `decode_object`, `decode_array`,
  `decode_value`, and `decode_slice`.
- Normalization and error handling are implemented in
  `src/internal/lenient_json_normalizer.rs` and `src/json_decode_error.rs`,
  which are covered by tests in `tests/`.
- Product requirements and implementation behavior are aligned with
  `doc/json_prd.zh_CN.md` and `doc/json_design.zh_CN.md`.

## Development Validation

Run the repository checks with `./align-ci.sh` followed by `./ci-check.sh`.
Criterion benchmarks are compiled with:

```bash
cargo bench --bench decoder_bench --no-run
```

The optional fuzz target is development tooling and is not a runtime
dependency. It exercises the default, strict, JSON-only, and required-closing
decoder policies. A bounded run is scheduled by `.github/workflows/fuzz.yml`;
install `cargo-fuzz` to build or run the same target locally from the
repository root:

```bash
cargo install cargo-fuzz
(cd fuzz && cargo fuzz build decoder)
(cd fuzz && cargo fuzz run decoder)
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

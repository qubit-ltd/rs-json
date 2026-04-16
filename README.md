# Qubit JSON

[![CircleCI](https://circleci.com/gh/qubit-ltd/rs-json.svg?style=shield)](https://circleci.com/gh/qubit-ltd/rs-json)
[![Coverage Status](https://coveralls.io/repos/github/qubit-ltd/rs-json/badge.svg?branch=main)](https://coveralls.io/github/qubit-ltd/rs-json?branch=main)
[![Crates.io](https://img.shields.io/crates/v/qubit-json.svg?color=blue)](https://crates.io/crates/qubit-json)
[![docs.rs](https://img.shields.io/docsrs/qubit-json?logo=docs.rs)](https://docs.rs/qubit-json)
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
- Markdown code blocks
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
- **Clear errors**: report stable error kinds with enough context for callers
- **Low overhead**: avoid unnecessary allocation when normalization can borrow
  the original input

## Features

### `LenientJsonDecoder`

- Reusable decoder object that holds immutable decoding options
- `decode<T>()`: decodes any JSON top-level value into `T`
- `decode_value()`: decodes into `serde_json::Value`
- `decode_object<T>()`: requires a top-level JSON object
- `decode_array<T>()`: requires a top-level JSON array

### `JsonDecodeOptions`

- `trim_whitespace`: trims leading and trailing whitespace
- `strip_utf8_bom`: strips a leading UTF-8 BOM
- `strip_markdown_code_fence`: strips one outer Markdown code fence
- `escape_control_chars_in_strings`: escapes ASCII control characters inside
  JSON string literals
- `max_input_bytes`: optional byte-size limit applied before normalization

### Explicit Error Model

- `InputTooLarge`: raw input size exceeds configured limit
- `EmptyInput`: input becomes empty after normalization
- `InvalidJson`: normalized text is not valid JSON syntax
- `UnexpectedTopLevel`: top-level JSON kind does not match the requested method
- `Deserialize`: JSON is valid but cannot be deserialized into the target type

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
qubit-json = "0.1.0"
```

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
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        strip_markdown_code_fence: false,
        ..JsonDecodeOptions::default()
    });

    let value = decoder
        .decode_value("{\"ok\":true}")
        .expect("plain JSON should still decode with custom options");

    assert_eq!(value["ok"], true);
}
```

## Normalization Rules

When enabled, the decoder applies the following pipeline before parsing:

1. validate that the input is not empty
2. trim surrounding whitespace
3. strip a leading UTF-8 BOM
4. strip one outer Markdown code fence
5. escape ASCII control characters inside JSON string literals

The decoder does not:

- add missing quotes
- add missing commas
- add missing braces or brackets
- rewrite arbitrary malformed JSON into guessed valid JSON

## When to Use

Qubit JSON is a good fit when:

- you need a reusable, configurable JSON decoder object
- your inputs are mostly valid JSON but may be wrapped or slightly noisy
- you want stable error categories around `serde_json`

It is not a good fit when:

- you need aggressive repair for heavily malformed JSON
- your inputs are not actually JSON
- a plain `serde_json::from_str()` call is already sufficient

## License

This project is licensed under the Apache 2.0 License. See [LICENSE](LICENSE)
for details.

## Alignment Notes

This README reflects the current object model:

- `LenientJsonDecoder` owns an internal `LenientJsonNormalizer`.
- Public decoding APIs are `decode`, `decode_object`, `decode_array`,
  `decode_value`.
- Normalization and error handling are implemented in
  `src/lenient_json_normalizer.rs`
  and `src/json_decode_error.rs`, which are covered by tests in `tests/`.
- Product requirements and implementation behavior are aligned with
  `doc/json_prd.zh_CN.md` and `doc/json_design.zh_CN.md`.

// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines stable semantic stages for JSON decoding failures.

use std::fmt;
use std::str::FromStr;

/// Identifies the semantic decoding stage where a failure occurred.
///
/// Stages describe public domain boundaries and do not expose whether the
/// scanner, normalizer, or Serde first detected a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonDecodeStage {
    /// Charging raw input bytes.
    Input,
    /// Decoding raw bytes as UTF-8 text.
    DecodeText,
    /// Normalizing text or charging normalized bytes.
    Normalize,
    /// Admitting decoded-value resources.
    Admission,
    /// Validating JSON syntax and numeric range.
    Parse,
    /// Enforcing a top-level object or array contract.
    TopLevelCheck,
    /// Materializing the requested Rust type.
    Deserialize,
}

impl fmt::Display for JsonDecodeStage {
    /// Writes the stable snake-case stage name.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Input => "input",
            Self::DecodeText => "decode_text",
            Self::Normalize => "normalize",
            Self::Admission => "admission",
            Self::Parse => "parse",
            Self::TopLevelCheck => "top_level_check",
            Self::Deserialize => "deserialize",
        })
    }
}

impl FromStr for JsonDecodeStage {
    type Err = &'static str;

    /// Parses a stable stage name without ASCII case sensitivity.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.eq_ignore_ascii_case("input") {
            Ok(Self::Input)
        } else if value.eq_ignore_ascii_case("decode_text") {
            Ok(Self::DecodeText)
        } else if value.eq_ignore_ascii_case("normalize") {
            Ok(Self::Normalize)
        } else if value.eq_ignore_ascii_case("admission") {
            Ok(Self::Admission)
        } else if value.eq_ignore_ascii_case("parse") {
            Ok(Self::Parse)
        } else if value.eq_ignore_ascii_case("top_level_check") {
            Ok(Self::TopLevelCheck)
        } else if value.eq_ignore_ascii_case("deserialize") {
            Ok(Self::Deserialize)
        } else {
            Err("unknown JsonDecodeStage")
        }
    }
}

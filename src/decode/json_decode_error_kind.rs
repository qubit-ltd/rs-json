// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines stable categories for JSON decoding failures.

use std::fmt;
use std::str::FromStr;

/// Represents the coarse category of a JSON decoding failure.
///
/// This exhaustive enum is the stable branching contract shared by strict and
/// normalizing decoders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonDecodeErrorKind {
    /// A configured resource budget rejected a measurement.
    Budget,
    /// Input was empty at a decoding or normalization boundary.
    EmptyInput,
    /// Raw byte input was not valid UTF-8.
    InvalidUtf8,
    /// Input was not one valid JSON document under the numeric contract.
    InvalidJson,
    /// A valid document had an unexpected top-level kind.
    UnexpectedTopLevel,
    /// A valid admitted document could not deserialize into the target type.
    Deserialize,
}

impl fmt::Display for JsonDecodeErrorKind {
    /// Writes the stable snake-case category name.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Budget => "budget",
            Self::EmptyInput => "empty_input",
            Self::InvalidUtf8 => "invalid_utf8",
            Self::InvalidJson => "invalid_json",
            Self::UnexpectedTopLevel => "unexpected_top_level",
            Self::Deserialize => "deserialize",
        })
    }
}

impl FromStr for JsonDecodeErrorKind {
    type Err = &'static str;

    /// Parses a stable category name without ASCII case sensitivity.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.eq_ignore_ascii_case("budget") {
            Ok(Self::Budget)
        } else if value.eq_ignore_ascii_case("empty_input") {
            Ok(Self::EmptyInput)
        } else if value.eq_ignore_ascii_case("invalid_utf8") {
            Ok(Self::InvalidUtf8)
        } else if value.eq_ignore_ascii_case("invalid_json") {
            Ok(Self::InvalidJson)
        } else if value.eq_ignore_ascii_case("unexpected_top_level") {
            Ok(Self::UnexpectedTopLevel)
        } else if value.eq_ignore_ascii_case("deserialize") {
            Ok(Self::Deserialize)
        } else {
            Err("unknown JsonDecodeErrorKind")
        }
    }
}

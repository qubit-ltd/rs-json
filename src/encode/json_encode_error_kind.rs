// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines stable categories for JSON encoding failures.

use std::fmt;
use std::str::FromStr;

/// Represents the coarse category of a JSON encoding failure.
///
/// This exhaustive enum is the stable branching contract for strict JSON
/// encoding. [`JsonEncodeError`](super::JsonEncodeError) keeps its internal
/// representation private so new implementation details do not affect callers
/// that branch through this category.
///
/// # Examples
///
/// ```
/// use qubit_json::encode::JsonEncodeErrorKind;
///
/// let kind = "invalid_raw_json".parse::<JsonEncodeErrorKind>()?;
/// assert_eq!(kind, JsonEncodeErrorKind::InvalidRawJson);
/// # Ok::<(), &'static str>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonEncodeErrorKind {
    /// Resource accounting rejected the value or output.
    Budget,
    /// A `RawValue` field did not contain valid JSON text.
    InvalidRawJson,
    /// Serde could not serialize the source value.
    Serialize,
    /// The external destination writer rejected output bytes.
    Write,
}

impl fmt::Display for JsonEncodeErrorKind {
    /// Writes the stable snake-case category name.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Budget => "budget",
            Self::InvalidRawJson => "invalid_raw_json",
            Self::Serialize => "serialize",
            Self::Write => "write",
        })
    }
}

impl FromStr for JsonEncodeErrorKind {
    type Err = &'static str;

    /// Parses a stable category name without ASCII case sensitivity.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.eq_ignore_ascii_case("budget") {
            Ok(Self::Budget)
        } else if value.eq_ignore_ascii_case("invalid_raw_json") {
            Ok(Self::InvalidRawJson)
        } else if value.eq_ignore_ascii_case("serialize") {
            Ok(Self::Serialize)
        } else if value.eq_ignore_ascii_case("write") {
            Ok(Self::Write)
        } else {
            Err("unknown JsonEncodeErrorKind")
        }
    }
}

// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines stable categories for JCS encoding failures.

use std::fmt;
use std::str::FromStr;

/// Stable category describing a failure while producing JCS output.
///
/// The string representation is stable and uses lowercase snake case, making
/// the kind suitable for logs and interchange formats.
///
/// # Examples
///
/// ```
/// use qubit_json::encode::JcsEncodeErrorKind;
/// use std::str::FromStr;
///
/// let kind = JcsEncodeErrorKind::from_str("working_memory")
///     .expect("known error kind");
/// assert_eq!(kind.to_string(), "working_memory");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JcsEncodeErrorKind {
    /// Resource accounting rejected the value or output.
    Budget,
    /// The encoder's auxiliary working memory exceeded its configured limit.
    WorkingMemory,
    /// JCS serialization could not represent the value.
    Serialize,
    /// The external destination rejected output bytes.
    Write,
}

impl fmt::Display for JcsEncodeErrorKind {
    /// Writes the stable lowercase snake-case category name.
    ///
    /// # Parameters
    ///
    /// * `formatter` - Destination that receives the category name.
    ///
    /// # Errors
    ///
    /// Returns the formatter's error if writing the name fails.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Budget => "budget",
            Self::WorkingMemory => "working_memory",
            Self::Serialize => "serialize",
            Self::Write => "write",
        })
    }
}

impl FromStr for JcsEncodeErrorKind {
    type Err = &'static str;

    /// Parses a category name without ASCII case sensitivity.
    ///
    /// # Parameters
    ///
    /// * `value` - Category name to parse.
    ///
    /// # Returns
    ///
    /// The matching JCS encoding error category.
    ///
    /// # Errors
    ///
    /// Returns a static diagnostic when `value` is not a known category.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.eq_ignore_ascii_case("budget") {
            Ok(Self::Budget)
        } else if value.eq_ignore_ascii_case("working_memory") {
            Ok(Self::WorkingMemory)
        } else if value.eq_ignore_ascii_case("serialize") {
            Ok(Self::Serialize)
        } else if value.eq_ignore_ascii_case("write") {
            Ok(Self::Write)
        } else {
            Err("unknown JcsEncodeErrorKind")
        }
    }
}

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

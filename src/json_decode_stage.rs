/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Defines the [`JsonDecodeStage`] type used by the public decoder API.
//!

use std::fmt;

/// Identifies the decoding stage where an error was produced.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonDecodeStage {
    /// The error happened while normalizing raw input text.
    Normalize,
    /// The error happened while parsing normalized text as JSON syntax.
    Parse,
    /// The error happened while enforcing a top-level kind contract.
    TopLevelCheck,
    /// The error happened while deserializing a parsed JSON value.
    Deserialize,
}

impl fmt::Display for JsonDecodeStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normalize => f.write_str("normalize"),
            Self::Parse => f.write_str("parse"),
            Self::TopLevelCheck => f.write_str("top_level_check"),
            Self::Deserialize => f.write_str("deserialize"),
        }
    }
}

// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the [`JsonDecodeStage`] type used by the public decoder API.

use std::fmt;

/// Identifies the decoding stage where an error was produced.
///
/// # Examples
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use qubit_json::JsonDecodeStage;
///
/// fn decode_stage() -> JsonDecodeStage {
///     JsonDecodeStage::Parse
/// }
///
/// decode_stage();
/// ```
#[must_use]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonDecodeStage {
    /// The error happened while decoding raw bytes as UTF-8 text.
    DecodeText,
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
    /// Writes the stable snake-case name of this decoder stage.
    ///
    /// # Parameters
    ///
    /// * `f` - Destination formatter.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the stage name is written successfully.
    ///
    /// # Errors
    ///
    /// Returns a formatting error when the destination rejects the write.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DecodeText => f.write_str("decode_text"),
            Self::Normalize => f.write_str("normalize"),
            Self::Parse => f.write_str("parse"),
            Self::TopLevelCheck => f.write_str("top_level_check"),
            Self::Deserialize => f.write_str("deserialize"),
        }
    }
}

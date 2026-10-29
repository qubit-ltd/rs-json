// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Errors returned by fallible lexical scanning.

use std::fmt;

use qubit_budget::ResourceQuantity;

use super::JsonLexicalError;

/// Classifies failures produced while scanning JSON for a consumer.
///
/// Lexical syntax and budget failures are carried by [`Self::Lexical`], while
/// callback failures are carried by [`Self::Consumer`]. An encoded string
/// limit violation has its own variant so callers can inspect its byte
/// measurements and input location.
#[derive(Debug)]
pub(crate) enum JsonLexicalScanError<E, R, Q>
where
    Q: ResourceQuantity,
{
    /// A scanner syntax or value-budget failure.
    Lexical(JsonLexicalError<R, Q>),
    /// The error returned by a consumer callback that stopped the scan.
    Consumer(E),
    /// A string token exceeded its maximum encoded source length.
    EncodedStringLimit {
        /// Maximum bytes between the string's quotes, including escape
        /// spelling.
        maximum: usize,
        /// Observed source bytes between the quotes, including escape spelling.
        observed: usize,
        /// Byte offset of the opening quote relative to the input start.
        offset: usize,
    },
}

/// Converts scanner syntax and budget failures into the lexical category.
impl<E, R, Q> From<JsonLexicalError<R, Q>> for JsonLexicalScanError<E, R, Q>
where
    Q: ResourceQuantity,
{
    fn from(error: JsonLexicalError<R, Q>) -> Self {
        Self::Lexical(error)
    }
}

/// Formats the contained lexical or consumer error, or a stable limit message.
impl<E, R, Q> fmt::Display for JsonLexicalScanError<E, R, Q>
where
    E: fmt::Display,
    R: fmt::Debug,
    Q: ResourceQuantity,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lexical(error) => write!(f, "{error:?}"),
            Self::Consumer(error) => error.fmt(f),
            Self::EncodedStringLimit { .. } => f.write_str("encoded JSON string limit exceeded"),
        }
    }
}

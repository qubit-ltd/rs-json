// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Error returned by the JCS JSON text decoder.

use std::error::Error;
use std::fmt;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonResource;

use super::JsonDecodeError;

/// JCS decoding failure with explicit duplicate-key and construction-depth
/// cases.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::{JcsDecodeError, JcsDecoder};
///
/// let error = JcsDecoder::unlimited()
///     .decode_str(r#"{"x":1,"x":2}"#)
///     .expect_err("duplicate keys must be rejected");
/// assert!(matches!(error, JcsDecodeError::DuplicateKey { .. }));
/// ```
#[must_use]
#[derive(Debug, Clone)]
pub enum JcsDecodeError<R = JsonResource, Q = usize>
where
    Q: Copy + fmt::Debug,
{
    /// An input, UTF-8, budget, or lexical admission failure.
    Decode(JsonDecodeError<R, Q>),
    /// An object contains two keys equal after JSON escape decoding.
    DuplicateKey {
        /// One-based source line of the second key token.
        line: usize,
        /// One-based UTF-8 character column of the second key token.
        column: usize,
    },
    /// A valid document exceeds the fixed JCS value construction depth.
    MaterializationDepthLimit {
        /// Maximum number of nested containers accepted during construction.
        limit: usize,
        /// One-based source line where the first over-limit container begins.
        line: usize,
        /// One-based UTF-8 character column of that container token.
        column: usize,
    },
}

impl<R, Q> fmt::Display for JcsDecodeError<R, Q>
where
    R: fmt::Debug,
    Q: ResourceQuantity,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => error.fmt(formatter),
            Self::DuplicateKey { line, column } => {
                write!(formatter, "duplicate JSON object key at line {line}, column {column}")
            }
            Self::MaterializationDepthLimit { limit, line, column } => write!(
                formatter,
                "JCS value construction depth exceeds {limit} at line {line}, column {column}"
            ),
        }
    }
}

impl<R, Q> Error for JcsDecodeError<R, Q>
where
    R: fmt::Debug + 'static,
    Q: ResourceQuantity + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            Self::DuplicateKey { .. } | Self::MaterializationDepthLimit { .. } => None,
        }
    }
}

impl<R, Q> From<JsonDecodeError<R, Q>> for JcsDecodeError<R, Q>
where
    Q: Copy + fmt::Debug,
{
    fn from(error: JsonDecodeError<R, Q>) -> Self {
        Self::Decode(error)
    }
}

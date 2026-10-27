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

/// JCS decoding failure with explicit duplicate-key handling.
///
/// # Type Parameters
///
/// * `R` - Resource identity used by the wrapped budget error.
/// * `Q` - Quantity representation used for resource accounting.
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
    Decode(
        /// Shared decode error with stable category and input diagnostics.
        JsonDecodeError<R, Q>,
    ),
    /// An object contains two keys equal after JSON escape decoding.
    DuplicateKey {
        /// One-based source line of the second key token.
        line: usize,
        /// One-based UTF-8 character column of the second key token.
        column: usize,
    },
    /// Historical compatibility variant; the shared builder no longer emits it.
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
    /// Formats the decoding failure with its source location when applicable.
    ///
    /// # Parameters
    ///
    /// * `formatter` - Destination that receives the formatted message.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the message is written.
    ///
    /// # Errors
    ///
    /// Returns the formatter error if writing the message fails.
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
    /// Returns the wrapped decoding error for the standard error chain.
    ///
    /// # Returns
    ///
    /// The wrapped decode error for `Decode`, or `None` for the JCS-specific
    /// duplicate-key and construction-depth failures.
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
    /// Wraps a regular decoding failure as a JCS decoding failure.
    ///
    /// # Parameters
    ///
    /// * `error` - Regular decoding failure to wrap.
    ///
    /// # Returns
    ///
    /// A JCS error containing the supplied decoding failure.
    fn from(error: JsonDecodeError<R, Q>) -> Self {
        Self::Decode(error)
    }
}

// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Structured errors from validated JSON decoding.

use std::error::Error;
use std::fmt;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonResource;

use super::JsonDecodeContext;
use super::JsonDecodeError;
use super::JsonNumberTokenError;
use super::JsonValidatedLimit;

/// A validated decoding failure with source offset and privacy-safe outer
/// formatting.
///
/// The `Policy` variant preserves the caller's error unchanged. Its contents
/// are omitted from this error's [`Display`](fmt::Display) and
/// [`Debug`](fmt::Debug) output, but are available through [`Error::source`].
/// This caller-provided source is outside [`super::DiagnosticPolicy`]'s
/// redaction boundary; callers must choose safe policy error contents and
/// logging behavior themselves.
///
/// ```
/// use qubit_json::decode::{
///     DiagnosticPolicy, JsonDecoder, JsonNumberPolicy,
///     JsonValidatedDecodeError, JsonValidatedDecodeOptions,
/// };
/// use std::error::Error;
///
/// let options = JsonValidatedDecodeOptions::builder().build();
/// let error = JsonDecoder::unlimited()
///     .with_diagnostic_policy(DiagnosticPolicy::Redacted)
///     .decode_unique_validated_str("1", &options, |_, token| {
///         Err::<JsonNumberPolicy, _>(std::io::Error::other(format!(
///             "policy-sensitive-marker:{}", token.raw(),
///         )))
///     })
///     .unwrap_err();
/// let sentinel = "policy-sensitive-marker";
/// assert!(!error.to_string().contains(sentinel));
/// assert!(!format!("{error:?}").contains(sentinel));
/// let source = Error::source(&error).expect("caller policy error is preserved");
/// assert!(source.to_string().contains(sentinel));
/// assert!(matches!(error, JsonValidatedDecodeError::Policy { .. }));
/// ```
#[non_exhaustive]
pub enum JsonValidatedDecodeError<E, R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    /// A decoder, syntax, budget, or input failure.
    Decode {
        /// Structured decoding failure.
        error: JsonDecodeError<R, Q>,
        /// Grammar location associated with the failure, when known.
        context: Option<JsonDecodeContext>,
    },
    /// The caller's number policy rejected a source token.
    Policy {
        /// Error returned by the caller's policy, preserved as the source.
        ///
        /// This value is not redacted by [`super::DiagnosticPolicy`]. Its
        /// formatting is omitted from the outer error's `Display` and `Debug`,
        /// but it remains accessible through [`Error::source`]. The caller is
        /// responsible for keeping sensitive data out of this error and its
        /// logs.
        source: E,
        /// Byte offset of the rejected number token.
        offset: usize,
    },
    /// Exact-safe-integer conversion failed.
    ExactSafeInteger {
        /// Exact conversion failure category.
        source: JsonNumberTokenError,
        /// Byte offset of the number token.
        offset: usize,
    },
    /// An additional validated decode limit was exceeded.
    Limit {
        /// Limit that rejected the document.
        limit: JsonValidatedLimit,
        /// Configured maximum.
        maximum: usize,
        /// Value observed when the limit was checked.
        observed: usize,
        /// Byte offset associated with the rejected token.
        offset: usize,
    },
}

impl<E, R, Q> fmt::Debug for JsonValidatedDecodeError<E, R, Q>
where
    R: fmt::Debug,
    Q: ResourceQuantity,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode { error, context } => f
                .debug_struct("Decode")
                .field("error", error)
                .field("context", context)
                .finish(),
            Self::Policy { offset, .. } => f.debug_struct("Policy").field("offset", offset).finish_non_exhaustive(),
            Self::ExactSafeInteger { source, offset } => f
                .debug_struct("ExactSafeInteger")
                .field("source", source)
                .field("offset", offset)
                .finish(),
            Self::Limit {
                limit,
                maximum,
                observed,
                offset,
            } => f
                .debug_struct("Limit")
                .field("limit", limit)
                .field("maximum", maximum)
                .field("observed", observed)
                .field("offset", offset)
                .finish(),
        }
    }
}

impl<E, R, Q> fmt::Display for JsonValidatedDecodeError<E, R, Q>
where
    R: fmt::Debug,
    Q: ResourceQuantity,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode { error, context } => write!(f, "JSON decode failed ({context:?}): {error}"),
            Self::Policy { offset, .. } => write!(f, "JSON number policy rejected token at byte {offset}"),
            Self::ExactSafeInteger { source, offset } => {
                write!(f, "exact safe integer conversion failed at byte {offset}: {source:?}")
            }
            Self::Limit {
                limit,
                maximum,
                observed,
                offset,
            } => write!(
                f,
                "JSON limit {limit:?} exceeded at byte {offset}: maximum {maximum}, observed {observed}"
            ),
        }
    }
}

impl<E, R, Q> Error for JsonValidatedDecodeError<E, R, Q>
where
    E: Error + 'static,
    R: fmt::Debug + 'static,
    Q: ResourceQuantity + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode { error, .. } => Some(error),
            Self::Policy { source, .. } => Some(source),
            Self::ExactSafeInteger { source, .. } => Some(source),
            Self::Limit { .. } => None,
        }
    }
}

//! Structured errors from validated JSON decoding.

use std::error::Error;
use std::fmt;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonResource;

use super::JsonDecodeContext;
use super::JsonDecodeError;
use super::JsonNumberTokenError;
use super::JsonValidatedLimit;

/// A validated decoding failure with source offset and privacy-safe display.
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
        /// Opaque error returned by the caller's policy.
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
            Self::Decode { error, context } => f.debug_struct("Decode").field("error", error).field("context", context).finish(),
            Self::Policy { offset, .. } => f.debug_struct("Policy").field("offset", offset).finish_non_exhaustive(),
            Self::ExactSafeInteger { source, offset } => f.debug_struct("ExactSafeInteger").field("source", source).field("offset", offset).finish(),
            Self::Limit { limit, maximum, observed, offset } => f.debug_struct("Limit").field("limit", limit).field("maximum", maximum).field("observed", observed).field("offset", offset).finish(),
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
            Self::ExactSafeInteger { source, offset } => write!(f, "exact safe integer conversion failed at byte {offset}: {source:?}"),
            Self::Limit { limit, maximum, observed, offset } => write!(f, "JSON limit {limit:?} exceeded at byte {offset}: maximum {maximum}, observed {observed}"),
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

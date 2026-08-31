// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines stable errors returned by strict JSON value encoding.

use std::fmt::Display;

use thiserror::Error;

use super::JsonCollectionKind;
use super::JsonIntegerSignedness;
use super::JsonMapKeyKind;
use super::JsonSerializerStateError;
use super::JsonValueEncodeErrorCategory;
use super::JsonValueEncodeErrorKind;

/// Privacy-safe failure produced while projecting a serializable value into
/// strict JSON.
///
/// The error exposes exact stable kinds and broad handling categories without
/// retaining input values, object keys, or arbitrary third-party diagnostics.
///
/// # Examples
///
/// ```
/// use qubit_json::value::JsonIntegerSignedness;
/// use qubit_json::value::JsonValueEncodeErrorKind;
/// use qubit_json::value::JsonValueEncoder;
///
/// let error = JsonValueEncoder::new()
///     .encode(&u128::MAX)
///     .expect_err("wide integer must be rejected");
/// assert_eq!(
///     error.kind(),
///     JsonValueEncodeErrorKind::IntegerOutOfRange {
///         signedness: JsonIntegerSignedness::Unsigned,
///     },
/// );
/// ```
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
#[error("{kind}")]
pub struct JsonValueEncodeError {
    /// Exact privacy-safe classification.
    kind: JsonValueEncodeErrorKind,
}

impl JsonValueEncodeError {
    /// Creates one internal error from its stable public kind.
    #[inline(always)]
    pub(crate) const fn new(kind: JsonValueEncodeErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the exact stable failure classification.
    #[must_use]
    #[inline(always)]
    pub const fn kind(&self) -> JsonValueEncodeErrorKind {
        self.kind
    }

    /// Returns the broad downstream handling category.
    #[must_use]
    pub const fn category(&self) -> JsonValueEncodeErrorCategory {
        match self.kind {
            JsonValueEncodeErrorKind::IntegerOutOfRange { .. }
            | JsonValueEncodeErrorKind::NonFiniteFloat
            | JsonValueEncodeErrorKind::InvalidNumberRepresentation => JsonValueEncodeErrorCategory::Number,
            JsonValueEncodeErrorKind::UnsupportedMapKey { .. } | JsonValueEncodeErrorKind::DuplicateObjectKey => {
                JsonValueEncodeErrorCategory::ObjectKey
            }
            JsonValueEncodeErrorKind::InvalidRawValue => JsonValueEncodeErrorCategory::RawValue,
            JsonValueEncodeErrorKind::CollectionLengthOverflow { .. } => JsonValueEncodeErrorCategory::Capacity,
            JsonValueEncodeErrorKind::InvalidSerializerState { .. }
            | JsonValueEncodeErrorKind::DisplayFormattingFailed => JsonValueEncodeErrorCategory::SerializerContract,
            JsonValueEncodeErrorKind::CustomSerialization => JsonValueEncodeErrorCategory::Custom,
        }
    }

    /// Reports whether this failure belongs to the strict number contract.
    #[must_use]
    #[inline(always)]
    pub const fn is_number_error(&self) -> bool {
        matches!(self.category(), JsonValueEncodeErrorCategory::Number)
    }

    /// Reports whether this failure concerns JSON object-key representation.
    #[must_use]
    #[inline(always)]
    pub const fn is_map_key_error(&self) -> bool {
        matches!(self.category(), JsonValueEncodeErrorCategory::ObjectKey)
    }

    /// Reports whether this failure concerns a RawValue payload.
    #[must_use]
    #[inline(always)]
    pub const fn is_raw_value_error(&self) -> bool {
        matches!(self.category(), JsonValueEncodeErrorCategory::RawValue)
    }

    /// Reports whether a hand-written serializer violated a protocol contract.
    #[must_use]
    #[inline(always)]
    pub const fn is_serializer_contract_error(&self) -> bool {
        matches!(self.category(), JsonValueEncodeErrorCategory::SerializerContract)
    }

    /// Returns the signedness of an out-of-range integer, when applicable.
    #[must_use]
    #[inline(always)]
    pub const fn integer_signedness(&self) -> Option<JsonIntegerSignedness> {
        match self.kind {
            JsonValueEncodeErrorKind::IntegerOutOfRange { signedness } => Some(signedness),
            _ => None,
        }
    }

    /// Returns the rejected map-key shape, when applicable.
    #[must_use]
    #[inline(always)]
    pub const fn map_key_kind(&self) -> Option<JsonMapKeyKind> {
        match self.kind {
            JsonValueEncodeErrorKind::UnsupportedMapKey { kind } => Some(kind),
            _ => None,
        }
    }

    /// Returns the collection whose count overflowed, when applicable.
    #[must_use]
    #[inline(always)]
    pub const fn collection_kind(&self) -> Option<JsonCollectionKind> {
        match self.kind {
            JsonValueEncodeErrorKind::CollectionLengthOverflow { kind } => Some(kind),
            _ => None,
        }
    }

    /// Returns the exact invalid serializer state, when applicable.
    #[must_use]
    #[inline(always)]
    pub const fn serializer_state_error(&self) -> Option<JsonSerializerStateError> {
        match self.kind {
            JsonValueEncodeErrorKind::InvalidSerializerState { reason } => Some(reason),
            _ => None,
        }
    }
}

impl serde::ser::Error for JsonValueEncodeError {
    /// Converts arbitrary custom serializer text into one opaque, stable kind.
    fn custom<T>(_message: T) -> Self
    where
        T: Display,
    {
        Self::new(JsonValueEncodeErrorKind::CustomSerialization)
    }
}

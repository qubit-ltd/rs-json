// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines stable, privacy-safe JSON serialization failures.

use std::fmt::Display;

use thiserror::Error;

use super::JsonCollectionKind;
use super::JsonIntegerSignedness;
use super::JsonMapKeyKind;
use super::JsonSerializationErrorCategory;
use super::JsonSerializationErrorKind;
use super::JsonSerializerStateError;

/// Privacy-safe failure produced while serializing a value as strict JSON.
///
/// The error exposes exact stable kinds and broad handling categories without
/// retaining input values, object keys, or arbitrary third-party diagnostics.
///
/// # Examples
///
/// ```
/// use qubit_json::encode::JsonIntegerSignedness;
/// use qubit_json::encode::JsonSerializationErrorKind;
/// use qubit_json::value::JsonValueEncoder;
///
/// let error = JsonValueEncoder::new()
///     .encode(&u128::MAX)
///     .expect_err("wide integer must be rejected");
/// assert_eq!(
///     error.kind(),
///     JsonSerializationErrorKind::IntegerOutOfRange {
///         signedness: JsonIntegerSignedness::Unsigned,
///     },
/// );
/// ```
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
#[error("{kind}")]
pub struct JsonSerializationError {
    /// Exact privacy-safe classification.
    kind: JsonSerializationErrorKind,
}

impl JsonSerializationError {
    /// Creates an error from its stable public kind.
    ///
    /// # Parameters
    ///
    /// * `kind` - Exact privacy-safe serialization failure.
    ///
    /// # Returns
    ///
    /// A serialization error retaining only `kind`.
    #[inline(always)]
    pub const fn new(kind: JsonSerializationErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the exact stable failure classification.
    ///
    /// # Returns
    ///
    /// The precise serialization failure kind retained by this error.
    #[must_use]
    #[inline(always)]
    pub const fn kind(&self) -> JsonSerializationErrorKind {
        self.kind
    }

    /// Returns the broad downstream handling category.
    ///
    /// # Returns
    ///
    /// The broad category that determines which class of serialization
    /// failure occurred.
    #[must_use]
    pub const fn category(&self) -> JsonSerializationErrorCategory {
        match self.kind {
            JsonSerializationErrorKind::IntegerOutOfRange { .. }
            | JsonSerializationErrorKind::NonFiniteFloat
            | JsonSerializationErrorKind::InvalidNumberRepresentation => JsonSerializationErrorCategory::Number,
            JsonSerializationErrorKind::UnsupportedMapKey { .. } | JsonSerializationErrorKind::DuplicateObjectKey => {
                JsonSerializationErrorCategory::ObjectKey
            }
            JsonSerializationErrorKind::InvalidRawValue => JsonSerializationErrorCategory::RawValue,
            JsonSerializationErrorKind::CollectionLengthOverflow { .. } => JsonSerializationErrorCategory::Capacity,
            JsonSerializationErrorKind::InvalidSerializerState { .. }
            | JsonSerializationErrorKind::DisplayFormattingFailed => JsonSerializationErrorCategory::SerializerContract,
            JsonSerializationErrorKind::CustomSerialization => JsonSerializationErrorCategory::Custom,
        }
    }

    /// Reports whether this failure belongs to the strict number contract.
    ///
    /// # Returns
    ///
    /// `true` when the failure concerns a JSON number; otherwise, `false`.
    #[must_use]
    #[inline(always)]
    pub const fn is_number_error(&self) -> bool {
        matches!(self.category(), JsonSerializationErrorCategory::Number)
    }

    /// Reports whether this failure concerns JSON object-key representation.
    ///
    /// # Returns
    ///
    /// `true` when the failure concerns an object key; otherwise, `false`.
    #[must_use]
    #[inline(always)]
    pub const fn is_map_key_error(&self) -> bool {
        matches!(self.category(), JsonSerializationErrorCategory::ObjectKey)
    }

    /// Reports whether this failure concerns a RawValue payload.
    ///
    /// # Returns
    ///
    /// `true` when the failure concerns a `RawValue` payload; otherwise,
    /// `false`.
    #[must_use]
    #[inline(always)]
    pub const fn is_raw_value_error(&self) -> bool {
        matches!(self.category(), JsonSerializationErrorCategory::RawValue)
    }

    /// Reports whether a hand-written serializer violated a protocol contract.
    ///
    /// # Returns
    ///
    /// `true` when a serializer or display implementation violated the
    /// compound protocol; otherwise, `false`.
    #[must_use]
    #[inline(always)]
    pub const fn is_serializer_contract_error(&self) -> bool {
        matches!(self.category(), JsonSerializationErrorCategory::SerializerContract)
    }

    /// Returns the signedness of an out-of-range integer, when applicable.
    ///
    /// # Returns
    ///
    /// `Some(signedness)` for an out-of-range integer, or `None` for every
    /// other serialization failure.
    #[must_use]
    #[inline(always)]
    pub const fn integer_signedness(&self) -> Option<JsonIntegerSignedness> {
        match self.kind {
            JsonSerializationErrorKind::IntegerOutOfRange { signedness } => Some(signedness),
            _ => None,
        }
    }

    /// Returns the rejected map-key shape, when applicable.
    #[must_use]
    #[inline(always)]
    pub const fn map_key_kind(&self) -> Option<JsonMapKeyKind> {
        match self.kind {
            JsonSerializationErrorKind::UnsupportedMapKey { kind } => Some(kind),
            _ => None,
        }
    }

    /// Returns the collection whose count overflowed, when applicable.
    #[must_use]
    #[inline(always)]
    pub const fn collection_kind(&self) -> Option<JsonCollectionKind> {
        match self.kind {
            JsonSerializationErrorKind::CollectionLengthOverflow { kind } => Some(kind),
            _ => None,
        }
    }

    /// Returns the exact invalid serializer state, when applicable.
    #[must_use]
    #[inline(always)]
    pub const fn serializer_state_error(&self) -> Option<JsonSerializerStateError> {
        match self.kind {
            JsonSerializationErrorKind::InvalidSerializerState { reason } => Some(reason),
            _ => None,
        }
    }
}

impl serde::ser::Error for JsonSerializationError {
    /// Converts arbitrary custom serializer text into one opaque, stable kind.
    fn custom<T>(_message: T) -> Self
    where
        T: Display,
    {
        Self::new(JsonSerializationErrorKind::CustomSerialization)
    }
}

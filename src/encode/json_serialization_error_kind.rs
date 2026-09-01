// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines precise, privacy-safe strict JSON serialization failures.

use thiserror::Error;

use super::JsonCollectionKind;
use super::JsonIntegerSignedness;
use super::JsonMapKeyKind;
use super::JsonSerializerStateError;

/// Precise reason why a Serde value could not become strict JSON.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum JsonSerializationErrorKind {
    /// A signed or unsigned integer exceeded the strict 64-bit union range.
    #[error("JSON integer is outside the supported 64-bit range")]
    IntegerOutOfRange {
        /// Signedness of the rejected Serde integer entry point.
        signedness: JsonIntegerSignedness,
    },
    /// A floating-point value was NaN or infinite.
    #[error("non-finite float")]
    NonFiniteFloat,
    /// A finite number unexpectedly could not become a serde_json number.
    #[error("invalid JSON number representation")]
    InvalidNumberRepresentation,
    /// A Serde value shape cannot be represented as an object key.
    #[error("unsupported JSON object key")]
    UnsupportedMapKey {
        /// Rejected non-sensitive Serde shape.
        kind: JsonMapKeyKind,
    },
    /// Two source entries produced the same JSON object key.
    #[error("duplicate JSON object key")]
    DuplicateObjectKey,
    /// A serde_json RawValue payload violated the strict value contract.
    #[error("invalid raw JSON value")]
    InvalidRawValue,
    /// A JSON collection count overflowed the platform representation.
    #[error("JSON collection length overflow")]
    CollectionLengthOverflow {
        /// Collection whose count overflowed.
        kind: JsonCollectionKind,
    },
    /// A hand-written Serialize implementation violated compound state rules.
    #[error("invalid serializer state")]
    InvalidSerializerState {
        /// Exact privacy-safe state violation.
        reason: JsonSerializerStateError,
    },
    /// A Display implementation rejected fallible formatting.
    #[error("display formatting failed during JSON serialization")]
    DisplayFormattingFailed,
    /// An external serializer returned opaque custom failure text.
    #[error("custom JSON serialization failed")]
    CustomSerialization,
}

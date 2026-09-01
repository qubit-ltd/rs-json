// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines broad handling categories for strict JSON value encoding failures.

/// Stable strategy category for a strict JSON value encoding failure.
///
/// # Examples
///
/// ```
/// use qubit_json::encode::JsonSerializationError;
/// use qubit_json::encode::JsonSerializationErrorCategory;
/// use qubit_json::encode::JsonSerializationErrorKind;
///
/// let error = JsonSerializationError::new(JsonSerializationErrorKind::NonFiniteFloat);
/// assert_eq!(error.category(), JsonSerializationErrorCategory::Number);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonSerializationErrorCategory {
    /// A numeric value violated the strict JSON number contract.
    Number,
    /// A JSON object key could not be represented unambiguously.
    ObjectKey,
    /// A RawValue payload violated the strict materialization contract.
    RawValue,
    /// A collection count exceeded the platform quantity representation.
    Capacity,
    /// A hand-written serializer or display implementation violated its
    /// contract.
    SerializerContract,
    /// A serializer returned an opaque custom failure.
    Custom,
}

// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines JSON collection kinds used by value-encoding errors.

/// JSON collection whose item count could not be represented.
///
/// # Examples
///
/// ```
/// use qubit_json::encode::JsonCollectionKind;
/// use qubit_json::encode::JsonSerializationError;
/// use qubit_json::encode::JsonSerializationErrorKind;
///
/// let error = JsonSerializationError::new(JsonSerializationErrorKind::CollectionLengthOverflow {
///     kind: JsonCollectionKind::Array,
/// });
/// assert_eq!(error.collection_kind(), Some(JsonCollectionKind::Array));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonCollectionKind {
    /// A JSON array.
    Array,
    /// A JSON object.
    Object,
}

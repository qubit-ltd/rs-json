// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Additional validated decoding limits.

/// Identifies an additional per-document decode limit for validated JSON.
///
/// The decoder reports this value in
/// [`JsonValidatedDecodeError::Limit`](crate::decode::JsonValidatedDecodeError::Limit)
/// when a configured value or encoded-string bound is exceeded.
///
/// # Examples
///
/// ```
/// use std::convert::Infallible;
///
/// use qubit_json::decode::JsonDecoder;
/// use qubit_json::decode::JsonNumberPolicy;
/// use qubit_json::decode::JsonValidatedDecodeError;
/// use qubit_json::decode::JsonValidatedDecodeOptions;
/// use qubit_json::decode::JsonValidatedLimit;
///
/// let options = JsonValidatedDecodeOptions::builder()
///     .max_value_and_key_nodes(2)
///     .build();
/// let error = JsonDecoder::unlimited()
///     .decode_unique_validated_str("{\"key\":1}", &options, |_, _| {
///         Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
///     })
///     .expect_err("the object, key, and value exceed the two-node limit");
/// assert!(matches!(
///     error,
///     JsonValidatedDecodeError::Limit {
///         limit: JsonValidatedLimit::ValueAndKeyNodes,
///         ..
///     }
/// ));
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonValidatedLimit {
    /// Value entries plus decoded object-key entries.
    ValueAndKeyNodes,
    /// Source bytes inside one encoded string token, excluding quotes.
    EncodedStringBytes,
}

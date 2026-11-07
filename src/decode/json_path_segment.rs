// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Typed location within a JSON document.

/// Identifies an object key or array index in a JSON value path.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::{JsonDecoder, JsonNumberPolicy, JsonPathSegment, JsonValidatedDecodeOptions};
/// use std::convert::Infallible;
///
/// let options = JsonValidatedDecodeOptions::builder().build();
/// let mut saw_id = false;
/// let result = JsonDecoder::unlimited().decode_unique_validated_str(
///     r#"{"id":42}"#,
///     &options,
///     |path, _token| {
///         saw_id = path == [JsonPathSegment::Key("id".to_owned())];
///         Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
///     },
/// )?;
/// assert!(saw_id);
/// assert_eq!(result.as_value()["id"].as_i64(), Some(42));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JsonPathSegment {
    /// An object key after JSON escape decoding.
    Key(
        /// The decoded key text, without its surrounding JSON string syntax.
        String,
    ),
    /// An array element index.
    Index(
        /// The zero-based position of the element within its containing array.
        usize,
    ),
}

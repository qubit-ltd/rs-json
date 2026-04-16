/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Defines the [`LenientJsonDecoder`] type and its public decoding methods.
//!
//! Author: Haixing Hu

use serde::de::DeserializeOwned;
use serde_json::{error::Category, Value};

use crate::{JsonDecodeError, JsonDecodeOptions, JsonTopLevelKind, LenientJsonNormalizer};

/// A configurable JSON decoder for non-fully-trusted text inputs.
///
/// `LenientJsonDecoder` applies a small set of predictable normalization rules
/// before delegating actual parsing and deserialization to `serde_json`.
///
/// The decoder itself is stateless aside from its immutable configuration, so a
/// single instance can be reused across many decoding calls.
#[derive(Debug, Clone, Default)]
pub struct LenientJsonDecoder {
    /// Stores the immutable normalization and decoding configuration used by
    /// this decoder instance.
    normalizer: LenientJsonNormalizer,
}

impl LenientJsonDecoder {
    /// Creates a decoder with the exact normalization rules in `options`.
    ///
    /// Reusing a decoder instance is recommended when multiple inputs should
    /// follow the same lenient decoding policy.
    #[must_use]
    pub const fn new(options: JsonDecodeOptions) -> Self {
        Self {
            normalizer: LenientJsonNormalizer::new(options),
        }
    }

    /// Returns the immutable options used by this decoder.
    ///
    /// This accessor allows callers to inspect the effective configuration
    /// without cloning the decoder or duplicating the options elsewhere.
    #[must_use]
    pub const fn options(&self) -> &JsonDecodeOptions {
        self.normalizer.options()
    }

    /// Decodes `input` into the target Rust type `T`.
    ///
    /// This method does not constrain the JSON top-level structure. Arrays,
    /// objects, scalars, and any other JSON value kinds are all allowed as long
    /// as they can be deserialized into `T`.
    ///
    /// The generic type `T` must implement [`DeserializeOwned`], because the
    /// decoder first builds an owned [`Value`] and then deserializes from it.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError`] when the input becomes empty after
    /// normalization, when the normalized text is not valid JSON, or when the
    /// parsed JSON value cannot be deserialized into `T`.
    pub fn decode<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let normalized = self.normalizer.normalize(input)?;
        serde_json::from_str(normalized.as_ref()).map_err(Self::map_decode_error)
    }

    /// Decodes `input` into a target type `T`, requiring a top-level JSON
    /// object.
    ///
    /// This method is useful for APIs that require a structured object at the
    /// top level and want an explicit error when an array or scalar is
    /// received.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError`] when the input cannot be normalized into a
    /// valid JSON value, when the top-level JSON kind is not an object, or
    /// when the object cannot be deserialized into `T`.
    pub fn decode_object<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let normalized = self.normalizer.normalize(input)?;
        self.ensure_top_level_from_text(normalized.as_ref(), JsonTopLevelKind::Object)?;
        serde_json::from_str(normalized.as_ref()).map_err(Self::map_decode_error)
    }

    /// Decodes `input` into a `Vec<T>`, requiring a top-level JSON array.
    ///
    /// This method should be preferred over [`Self::decode`] when the caller
    /// wants an explicit top-level array contract instead of relying on the
    /// target type alone.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError`] when the input cannot be normalized into a
    /// valid JSON value, when the top-level JSON kind is not an array, or when
    /// the array cannot be deserialized into `Vec<T>`.
    pub fn decode_array<T>(&self, input: &str) -> Result<Vec<T>, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let normalized = self.normalizer.normalize(input)?;
        self.ensure_top_level_from_text(normalized.as_ref(), JsonTopLevelKind::Array)?;
        serde_json::from_str(normalized.as_ref()).map_err(Self::map_decode_error)
    }

    /// Decodes `input` into a [`serde_json::Value`].
    ///
    /// This is the lowest-level public entry point. It exposes the normalized
    /// and parsed JSON value before any additional type-specific
    /// deserialization is attempted.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError`] when the input is empty after normalization
    /// or when the normalized text is not valid JSON syntax.
    pub fn decode_value(&self, input: &str) -> Result<Value, JsonDecodeError> {
        let normalized = self.normalizer.normalize(input)?;
        serde_json::from_str(normalized.as_ref()).map_err(JsonDecodeError::invalid_json)
    }

    /// Verifies that the normalized text starts with the required top-level
    /// JSON kind token, when such a token can be classified cheaply.
    fn ensure_top_level_from_text(
        &self,
        normalized: &str,
        expected: JsonTopLevelKind,
    ) -> Result<(), JsonDecodeError> {
        if let Some(actual) = Self::classify_top_level_from_text(normalized) {
            if actual != expected {
                return Err(JsonDecodeError::unexpected_top_level(expected, actual));
            }
        }
        Ok(())
    }

    /// Classifies the top-level JSON kind from the first significant character.
    ///
    /// Returns `None` when the first non-whitespace character is missing or not
    /// a valid JSON token start, in which case full parsing should handle the
    /// error mapping.
    fn classify_top_level_from_text(input: &str) -> Option<JsonTopLevelKind> {
        let first = input.chars().find(|ch| !ch.is_whitespace())?;
        match first {
            '{' => Some(JsonTopLevelKind::Object),
            '[' => Some(JsonTopLevelKind::Array),
            '"' | '-' | '0'..='9' | 't' | 'f' | 'n' => Some(JsonTopLevelKind::Other),
            _ => None,
        }
    }

    /// Maps one `serde_json` error from direct typed decoding to the crate
    /// error model.
    ///
    /// Syntax, EOF, and I/O categories are treated as invalid JSON input.
    /// Data category errors are treated as type deserialization failures.
    fn map_decode_error(error: serde_json::Error) -> JsonDecodeError {
        match error.classify() {
            Category::Data => JsonDecodeError::deserialize(error),
            Category::Io | Category::Syntax | Category::Eof => JsonDecodeError::invalid_json(error),
        }
    }
}

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
use serde_json::{Value, error::Category};

use crate::{
    JsonDecodeError, JsonDecodeOptions, JsonTopLevelKind,
    lenient_json_normalizer::LenientJsonNormalizer,
};

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
    /// The generic type `T` must implement [`DeserializeOwned`] because this
    /// method deserializes directly from normalized text and does not return
    /// values borrowing from the input.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError`] when the input becomes empty after
    /// normalization, when the normalized text is not valid JSON, or when the
    /// parsed JSON value cannot be deserialized into `T`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qubit_json::LenientJsonDecoder;
    ///
    /// let decoder = LenientJsonDecoder::default();
    /// let value: u64 = decoder
    ///     .decode("42")
    ///     .expect("a numeric JSON scalar should decode into u64");
    ///
    /// assert_eq!(value, 42);
    /// ```
    pub fn decode<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let normalized = self.normalizer.normalize(input)?;
        serde_json::from_str(normalized.as_ref())
            .map_err(|error| Self::map_decode_error(error, normalized.len()))
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qubit_json::LenientJsonDecoder;
    ///
    /// let decoder = LenientJsonDecoder::default();
    /// let value: serde_json::Value = decoder
    ///     .decode_object("```json\n{\"ok\":true}\n```")
    ///     .expect("a fenced JSON object should decode into a value");
    ///
    /// assert_eq!(value["ok"], true);
    /// ```
    pub fn decode_object<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let normalized = self.normalizer.normalize(input)?;
        let value = Self::parse_value(normalized.as_ref())?;
        Self::ensure_top_level_from_value(&value, JsonTopLevelKind::Object)?;
        serde_json::from_value(value)
            .map_err(|error| JsonDecodeError::deserialize(error, Some(normalized.len())))
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qubit_json::{JsonDecodeErrorKind, LenientJsonDecoder};
    ///
    /// let decoder = LenientJsonDecoder::default();
    /// let error = decoder
    ///     .decode_array::<serde_json::Value>("{\"ok\":true}")
    ///     .expect_err("a top-level object should fail an array contract");
    ///
    /// assert_eq!(error.kind, JsonDecodeErrorKind::UnexpectedTopLevel);
    /// ```
    pub fn decode_array<T>(&self, input: &str) -> Result<Vec<T>, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let normalized = self.normalizer.normalize(input)?;
        let value = Self::parse_value(normalized.as_ref())?;
        Self::ensure_top_level_from_value(&value, JsonTopLevelKind::Array)?;
        serde_json::from_value(value)
            .map_err(|error| JsonDecodeError::deserialize(error, Some(normalized.len())))
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use qubit_json::{JsonDecodeOptions, LenientJsonDecoder};
    ///
    /// let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
    ///     max_input_bytes: Some(16),
    ///     ..JsonDecodeOptions::default()
    /// });
    /// let value = decoder
    ///     .decode_value("{\"ok\":true}")
    ///     .expect("input within the size limit should decode");
    ///
    /// assert_eq!(value["ok"], true);
    /// ```
    pub fn decode_value(&self, input: &str) -> Result<Value, JsonDecodeError> {
        let normalized = self.normalizer.normalize(input)?;
        Self::parse_value(normalized.as_ref())
    }

    /// Parses normalized text into a JSON value.
    ///
    /// Syntax failures are mapped to the crate error model with normalized
    /// input byte length included for diagnostics.
    fn parse_value(normalized: &str) -> Result<Value, JsonDecodeError> {
        serde_json::from_str(normalized)
            .map_err(|error| JsonDecodeError::invalid_json(error, Some(normalized.len())))
    }

    /// Verifies that a parsed JSON value has the required top-level kind.
    fn ensure_top_level_from_value(
        value: &Value,
        expected: JsonTopLevelKind,
    ) -> Result<(), JsonDecodeError> {
        let actual = JsonTopLevelKind::of(value);
        if actual != expected {
            return Err(JsonDecodeError::unexpected_top_level(expected, actual));
        }
        Ok(())
    }

    /// Maps one `serde_json` error from direct typed decoding to the crate
    /// error model.
    ///
    /// Syntax, EOF, and I/O categories are treated as invalid JSON input.
    /// Data category errors are treated as type deserialization failures.
    fn map_decode_error(error: serde_json::Error, input_bytes: usize) -> JsonDecodeError {
        match error.classify() {
            Category::Data => JsonDecodeError::deserialize(error, Some(input_bytes)),
            Category::Io | Category::Syntax | Category::Eof => {
                JsonDecodeError::invalid_json(error, Some(input_bytes))
            }
        }
    }
}

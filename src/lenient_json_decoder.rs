// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the [`LenientJsonDecoder`] type and its public decoding methods.

use serde::de::DeserializeOwned;
use serde_json::Error;
use serde_json::Value;
use serde_json::error::Category;
use serde_json::from_str;
use serde_json::value::RawValue;

use crate::ErrorPrivacyPolicy;
use crate::JsonDecodeError;
use crate::JsonDecodeOptions;
use crate::JsonTopLevelKind;
use crate::internal::lenient_json_normalizer::LenientJsonNormalizer;

/// A configurable JSON decoder for non-fully-trusted text inputs.
///
/// `LenientJsonDecoder` applies a small set of predictable normalization rules
/// before delegating actual parsing and deserialization to `serde_json`.
#[must_use = "a JSON decoder must be used to decode input"]
#[derive(Debug, Clone, Default)]
pub struct LenientJsonDecoder {
    /// Stores the configured normalization pipeline.
    normalizer: LenientJsonNormalizer,
}

impl LenientJsonDecoder {
    /// Creates a decoder with the exact normalization rules in `options`.
    ///
    /// # Parameters
    ///
    /// * `options` - Immutable normalization and error-diagnostic options.
    ///
    /// # Returns
    ///
    /// A decoder configured with `options`.
    #[inline(always)]
    pub const fn new(options: JsonDecodeOptions) -> Self {
        Self {
            normalizer: LenientJsonNormalizer::new(options),
        }
    }

    /// Returns the immutable options used by this decoder.
    ///
    /// # Returns
    ///
    /// The option set supplied when the decoder was created.
    #[inline(always)]
    #[must_use = "the decoder options should be inspected or retained"]
    pub const fn options(&self) -> &JsonDecodeOptions {
        self.normalizer.options()
    }

    /// Decodes `input` into the target Rust type `T` without a top-level
    /// structure constraint.
    ///
    /// # Parameters
    ///
    /// * `input` - Raw JSON text to normalize and deserialize.
    ///
    /// # Returns
    ///
    /// The deserialized target value.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError`] when input normalization, JSON parsing, or
    /// target deserialization fails.
    ///
    /// # Panics
    ///
    /// Panics when the [`serde::Deserialize`] implementation for `T` panics.
    pub fn decode<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().error_privacy_policy();
        let normalized = self.normalizer.normalize(input)?;
        Self::deserialize_normalized(
            normalized.as_ref(),
            raw_input_bytes,
            normalized.len(),
            privacy_policy,
        )
    }

    /// Decodes UTF-8 input bytes into the target Rust type.
    ///
    /// The configured raw byte limit is enforced before UTF-8 validation.
    /// Valid UTF-8 is borrowed and delegated to the string decoder.
    ///
    /// # Parameters
    ///
    /// * `input` - Raw JSON bytes to validate, normalize, and deserialize.
    ///
    /// # Returns
    ///
    /// The deserialized target value.
    ///
    /// # Errors
    ///
    /// Returns a JSON decode error when the raw byte limit is exceeded, the
    /// bytes are not valid UTF-8, or subsequent JSON decoding fails.
    ///
    /// # Panics
    ///
    /// Panics when the [`serde::Deserialize`] implementation for `T` panics.
    pub fn decode_slice<T>(&self, input: &[u8]) -> Result<T, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().error_privacy_policy();
        if let Some(max_input_bytes) = self.options().max_input_bytes()
            && raw_input_bytes > max_input_bytes
        {
            return Err(JsonDecodeError::input_too_large(
                raw_input_bytes,
                max_input_bytes,
                privacy_policy,
            ));
        }
        let input = std::str::from_utf8(input).map_err(|error| {
            JsonDecodeError::invalid_utf8(error, raw_input_bytes, privacy_policy)
        })?;
        self.decode(input)
    }

    /// Decodes `input` into `T`, requiring a top-level JSON object.
    ///
    /// The target is deserialized directly from normalized text after a
    /// top-level check, preserving serde's duplicate-field and number handling
    /// semantics.
    ///
    /// # Parameters
    ///
    /// * `input` - Raw JSON text to normalize and deserialize.
    ///
    /// # Returns
    ///
    /// The deserialized target value when the normalized input is a JSON
    /// object.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError`] when normalization or parsing fails, when
    /// the top-level value is not an object, or when the object cannot be
    /// deserialized into `T`.
    ///
    /// # Panics
    ///
    /// Panics when the [`serde::Deserialize`] implementation for `T` panics.
    #[inline(always)]
    pub fn decode_object<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        self.decode_with_top_level(input, JsonTopLevelKind::Object)
    }

    /// Decodes `input` into `Vec<T>`, requiring a top-level JSON array.
    ///
    /// The elements are deserialized directly from normalized text after a
    /// top-level check.
    ///
    /// # Parameters
    ///
    /// * `input` - Raw JSON text to normalize and deserialize.
    ///
    /// # Returns
    ///
    /// The deserialized elements when the normalized input is a JSON array.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError`] when normalization or parsing fails, when
    /// the top-level value is not an array, or when an element cannot be
    /// deserialized into `T`.
    ///
    /// # Panics
    ///
    /// Panics when the [`serde::Deserialize`] implementation for `T` panics.
    #[inline(always)]
    pub fn decode_array<T>(&self, input: &str) -> Result<Vec<T>, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        self.decode_with_top_level(input, JsonTopLevelKind::Array)
    }

    /// Decodes `input` into a [`serde_json::Value`].
    ///
    /// This entry point intentionally constructs a JSON DOM because its public
    /// return type is [`Value`].
    ///
    /// # Parameters
    ///
    /// * `input` - Raw JSON text to normalize and parse.
    ///
    /// # Returns
    ///
    /// The parsed dynamic JSON value.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError`] when input normalization or JSON parsing
    /// fails.
    pub fn decode_value(&self, input: &str) -> Result<Value, JsonDecodeError> {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().error_privacy_policy();
        let normalized = self.normalizer.normalize(input)?;
        Self::parse_value(
            normalized.as_ref(),
            raw_input_bytes,
            normalized.len(),
            privacy_policy,
        )
    }

    /// Decodes input while enforcing an object or array top-level contract.
    ///
    /// # Parameters
    ///
    /// * `input` - Raw JSON text to normalize and deserialize.
    /// * `expected` - Required top-level JSON kind.
    ///
    /// # Returns
    ///
    /// The deserialized target value.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError`] when normalization or parsing fails, when
    /// the validated top-level kind differs from `expected`, or when target
    /// deserialization fails.
    ///
    /// # Panics
    ///
    /// Panics from `T`'s `Deserialize` implementation or visitor methods are
    /// not caught and propagate to the caller.
    fn decode_with_top_level<T>(
        &self,
        input: &str,
        expected: JsonTopLevelKind,
    ) -> Result<T, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().error_privacy_policy();
        let normalized = self.normalizer.normalize(input)?;
        let normalized_input_bytes = normalized.len();
        let actual = JsonTopLevelKind::of_normalized_json(normalized.as_ref());
        if actual != expected {
            Self::validate_json(
                normalized.as_ref(),
                raw_input_bytes,
                normalized_input_bytes,
                privacy_policy,
            )?;
            return Err(JsonDecodeError::unexpected_top_level(
                expected,
                actual,
                raw_input_bytes,
                normalized_input_bytes,
                privacy_policy,
            ));
        }
        Self::deserialize_normalized(
            normalized.as_ref(),
            raw_input_bytes,
            normalized_input_bytes,
            privacy_policy,
        )
    }

    /// Parses normalized JSON text into a dynamic value.
    ///
    /// # Parameters
    ///
    /// * `normalized` - Normalized JSON text.
    /// * `raw_input_bytes` - Input length before normalization.
    /// * `normalized_input_bytes` - Normalized text length.
    /// * `privacy_policy` - Policy applied to parse diagnostics.
    ///
    /// # Returns
    ///
    /// The parsed dynamic JSON value.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeErrorKind::InvalidJson`](crate::JsonDecodeErrorKind::InvalidJson)
    /// when `normalized` is not valid JSON.
    #[inline]
    fn parse_value(
        normalized: &str,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Result<Value, JsonDecodeError> {
        from_str(normalized).map_err(|error| {
            JsonDecodeError::invalid_json(
                error,
                raw_input_bytes,
                normalized_input_bytes,
                privacy_policy,
            )
        })
    }

    /// Validates normalized JSON syntax without constructing a value tree.
    ///
    /// # Parameters
    ///
    /// * `normalized` - Normalized JSON text.
    /// * `raw_input_bytes` - Input length before normalization.
    /// * `normalized_input_bytes` - Normalized text length.
    /// * `privacy_policy` - Policy applied to parse diagnostics.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the complete normalized text is valid JSON.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeErrorKind::InvalidJson`](crate::JsonDecodeErrorKind::InvalidJson)
    /// when validation fails.
    #[inline]
    fn validate_json(
        normalized: &str,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Result<(), JsonDecodeError> {
        let _: &RawValue = from_str(normalized).map_err(|error| {
            JsonDecodeError::invalid_json(
                error,
                raw_input_bytes,
                normalized_input_bytes,
                privacy_policy,
            )
        })?;
        Ok(())
    }

    /// Deserializes normalized JSON text into `T`.
    ///
    /// # Parameters
    ///
    /// * `normalized` - Normalized JSON text.
    /// * `raw_input_bytes` - Input length before normalization.
    /// * `normalized_input_bytes` - Normalized text length.
    /// * `privacy_policy` - Policy applied to decode diagnostics.
    ///
    /// # Returns
    ///
    /// The deserialized target value.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError`] classified as invalid JSON for syntax and
    /// end-of-input failures. A data error is classified as a deserialization
    /// failure only when complete syntax validation succeeds.
    ///
    /// # Panics
    ///
    /// Panics from `T`'s `Deserialize` implementation or visitor methods are
    /// not caught and propagate to the caller.
    #[inline]
    fn deserialize_normalized<T>(
        normalized: &str,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Result<T, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        from_str(normalized).map_err(|error| {
            Self::map_decode_error(
                normalized,
                error,
                raw_input_bytes,
                normalized_input_bytes,
                privacy_policy,
            )
        })
    }

    /// Maps a serde error to the stable public decoder error model.
    ///
    /// # Parameters
    ///
    /// * `normalized` - Complete normalized JSON text.
    /// * `error` - Serde JSON error to classify.
    /// * `raw_input_bytes` - Input length before normalization.
    /// * `normalized_input_bytes` - Normalized text length.
    /// * `privacy_policy` - Policy applied to retained diagnostics.
    ///
    /// # Returns
    ///
    /// A deserialization error for data failures in otherwise valid JSON, or
    /// an invalid-JSON error when complete syntax validation fails.
    #[must_use]
    fn map_decode_error(
        normalized: &str,
        error: Error,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> JsonDecodeError {
        match error.classify() {
            Category::Data => match Self::validate_json(
                normalized,
                raw_input_bytes,
                normalized_input_bytes,
                privacy_policy,
            ) {
                Ok(()) => JsonDecodeError::deserialize(
                    error,
                    raw_input_bytes,
                    normalized_input_bytes,
                    privacy_policy,
                ),
                Err(error) => error,
            },
            Category::Io | Category::Syntax | Category::Eof => JsonDecodeError::invalid_json(
                error,
                raw_input_bytes,
                normalized_input_bytes,
                privacy_policy,
            ),
        }
    }
}

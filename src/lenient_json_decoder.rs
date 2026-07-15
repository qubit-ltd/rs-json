// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the [`LenientJsonDecoder`] type and its public decoding methods.

use serde::de::DeserializeOwned;
use serde_json::{
    Value,
    error::Category,
    value::RawValue,
};

use crate::{
    JsonDecodeError,
    JsonDecodeOptions,
    JsonTopLevelKind,
    lenient_json_normalizer::LenientJsonNormalizer,
};

/// A configurable JSON decoder for non-fully-trusted text inputs.
///
/// `LenientJsonDecoder` applies a small set of predictable normalization rules
/// before delegating actual parsing and deserialization to `serde_json`.
#[derive(Debug, Clone, Default)]
pub struct LenientJsonDecoder {
    normalizer: LenientJsonNormalizer,
}

impl LenientJsonDecoder {
    /// Creates a decoder with the exact normalization rules in `options`.
    #[must_use]
    pub const fn new(options: JsonDecodeOptions) -> Self {
        Self {
            normalizer: LenientJsonNormalizer::new(options),
        }
    }

    /// Returns the immutable options used by this decoder.
    #[must_use]
    pub const fn options(&self) -> &JsonDecodeOptions {
        self.normalizer.options()
    }

    /// Decodes `input` into the target Rust type `T` without a top-level
    /// structure constraint.
    pub fn decode<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let raw_input_bytes = input.len();
        let normalized = self.normalizer.normalize(input)?;
        Self::deserialize_normalized(
            normalized.as_ref(),
            raw_input_bytes,
            normalized.len(),
        )
    }

    /// Decodes `input` into `T`, requiring a top-level JSON object.
    ///
    /// The target is deserialized directly from normalized text after syntax
    /// and top-level validation, preserving serde's duplicate-field and number
    /// handling semantics.
    pub fn decode_object<T>(&self, input: &str) -> Result<T, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        self.decode_with_top_level(input, JsonTopLevelKind::Object)
    }

    /// Decodes `input` into `Vec<T>`, requiring a top-level JSON array.
    ///
    /// The elements are deserialized directly from normalized text after syntax
    /// and top-level validation.
    pub fn decode_array<T>(
        &self,
        input: &str,
    ) -> Result<Vec<T>, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        self.decode_with_top_level(input, JsonTopLevelKind::Array)
    }

    /// Decodes `input` into a [`serde_json::Value`].
    ///
    /// This entry point intentionally constructs a JSON DOM because its public
    /// return type is [`Value`].
    pub fn decode_value(&self, input: &str) -> Result<Value, JsonDecodeError> {
        let raw_input_bytes = input.len();
        let normalized = self.normalizer.normalize(input)?;
        Self::parse_value(
            normalized.as_ref(),
            raw_input_bytes,
            normalized.len(),
        )
    }

    fn decode_with_top_level<T>(
        &self,
        input: &str,
        expected: JsonTopLevelKind,
    ) -> Result<T, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let raw_input_bytes = input.len();
        let normalized = self.normalizer.normalize(input)?;
        let normalized_input_bytes = normalized.len();
        Self::validate_json(
            normalized.as_ref(),
            raw_input_bytes,
            normalized_input_bytes,
        )?;
        let actual = JsonTopLevelKind::of_normalized_json(normalized.as_ref());
        if actual != expected {
            return Err(JsonDecodeError::unexpected_top_level(
                expected,
                actual,
                raw_input_bytes,
                normalized_input_bytes,
            ));
        }
        Self::deserialize_normalized(
            normalized.as_ref(),
            raw_input_bytes,
            normalized_input_bytes,
        )
    }

    fn parse_value(
        normalized: &str,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
    ) -> Result<Value, JsonDecodeError> {
        serde_json::from_str(normalized).map_err(|error| {
            JsonDecodeError::invalid_json(
                error,
                raw_input_bytes,
                normalized_input_bytes,
            )
        })
    }

    fn validate_json(
        normalized: &str,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
    ) -> Result<(), JsonDecodeError> {
        let _: &RawValue =
            serde_json::from_str(normalized).map_err(|error| {
                JsonDecodeError::invalid_json(
                    error,
                    raw_input_bytes,
                    normalized_input_bytes,
                )
            })?;
        Ok(())
    }

    fn deserialize_normalized<T>(
        normalized: &str,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
    ) -> Result<T, JsonDecodeError>
    where
        T: DeserializeOwned,
    {
        serde_json::from_str(normalized).map_err(|error| {
            Self::map_decode_error(
                error,
                raw_input_bytes,
                normalized_input_bytes,
            )
        })
    }

    fn map_decode_error(
        error: serde_json::Error,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
    ) -> JsonDecodeError {
        match error.classify() {
            Category::Data => JsonDecodeError::deserialize(
                error,
                raw_input_bytes,
                normalized_input_bytes,
            ),
            Category::Io | Category::Syntax | Category::Eof => {
                JsonDecodeError::invalid_json(
                    error,
                    raw_input_bytes,
                    normalized_input_bytes,
                )
            }
        }
    }
}

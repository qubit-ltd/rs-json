// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the [`NormalizingJsonDecoder`] type and its public decoding methods.

use qubit_budget::ResourceBudget;
use qubit_budget::json::JsonDecodeAttempt;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use serde::de::DeserializeOwned;
use serde_json::Error;
use serde_json::Value;
use serde_json::error::Category;
use serde_json::from_str;
use serde_json::value::RawValue;

use super::DiagnosticPolicy;
use super::JsonRootKind;
use super::NormalizingJsonDecodeError;
use super::NormalizingJsonDecodeOptions;
use super::internal::json_normalizer::JsonNormalizer;
use crate::lexical::JsonLexicalError;
use crate::lexical::JsonLexicalScanner;

/// A configurable JSON decoder for non-fully-trusted text inputs.
///
/// `NormalizingJsonDecoder` applies a small set of predictable normalization
/// rules before delegating actual parsing and deserialization to `serde_json`.
#[derive(Debug, Clone, Default)]
pub struct NormalizingJsonDecoder {
    /// Stores the configured normalization pipeline.
    normalizer: JsonNormalizer,
}

impl NormalizingJsonDecoder {
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
    #[must_use]
    pub const fn new(options: NormalizingJsonDecodeOptions) -> Self {
        Self {
            normalizer: JsonNormalizer::new(options),
        }
    }

    /// Returns the immutable options used by this decoder.
    ///
    /// # Returns
    ///
    /// The option set supplied when the decoder was created.
    #[inline(always)]
    #[must_use]
    pub const fn options(&self) -> &NormalizingJsonDecodeOptions {
        self.normalizer.options()
    }

    /// Decodes `input` into the target Rust type `T` without a top-level
    /// structure constraint.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Target type deserialized from the normalized JSON text.
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
    /// Returns [`NormalizingJsonDecodeError`] when input normalization, JSON
    /// parsing, or target deserialization fails.
    ///
    /// # Panics
    ///
    /// Panics when the [`serde::Deserialize`] implementation for `T` panics.
    pub fn decode<T>(
        &self,
        input: &str,
    ) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        self.normalize_then_deserialize(input)
    }

    /// Decodes `input` while charging a caller-owned JSON decode session.
    ///
    /// Raw bytes are charged before normalization, normalized bytes are charged
    /// by the normalizer, and decoded value resources are staged by lexical
    /// admission. Raw and normalized input charges remain in the supplied
    /// reusable session after failure, while value charges are committed only
    /// for a successfully deserialized top-level value.
    ///
    /// # Parameters
    ///
    /// * `input` - Raw JSON text to normalize, admit, and deserialize.
    /// * `session` - Reusable caller-owned accounting state. Input charges are
    ///   retained on both success and failure; value charges are retained only
    ///   after successful top-level decoding.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Target type deserialized from the admitted JSON text.
    ///
    /// # Returns
    ///
    /// The directly deserialized target value after all configured resources
    /// are admitted.
    ///
    /// # Errors
    ///
    /// Returns [`NormalizingJsonDecodeError`] when normalization fails, lexical
    /// admission rejects syntax or a value resource, or the admitted JSON
    /// cannot be deserialized into `T`. Value-resource failures use the
    /// stable `Budget`/`Admission` classification and retain the measured
    /// rejection.
    ///
    /// # Panics
    ///
    /// Panics when the [`serde::Deserialize`] implementation for `T` panics.
    pub fn decode_with_session<T>(
        &self,
        input: &str,
        session: &mut JsonDecodeSession<'_, JsonResource>,
    ) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().diagnostic_policy();
        let mut attempt = session.begin_value();
        let normalized = self.normalizer.normalize(input, &mut attempt)?;
        JsonLexicalScanner::new(attempt.value_transaction_mut())
            .scan(normalized.as_bytes())
            .map_err(|error| {
                Self::map_admission_error(
                    error,
                    normalized.as_ref(),
                    raw_input_bytes,
                    privacy_policy,
                )
            })?;
        let value = Self::deserialize_normalized(
            normalized.as_ref(),
            raw_input_bytes,
            normalized.len(),
            privacy_policy,
        )?;
        attempt.commit();
        Ok(value)
    }

    /// Decodes one complete JSON string with a caller-owned session.
    #[inline]
    pub fn decode_str<T>(
        &self,
        input: &str,
        session: &mut JsonDecodeSession<'_, JsonResource>,
    ) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        self.decode_with_session(input, session)
    }

    /// Decodes one UTF-8 byte slice with a caller-owned session.
    pub fn decode_utf8<T>(
        &self,
        input: &[u8],
        session: &mut JsonDecodeSession<'_, JsonResource>,
    ) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().diagnostic_policy();
        let mut attempt = session.begin_value();
        attempt
            .try_consume_input_bytes(raw_input_bytes)
            .map_err(|_| {
                NormalizingJsonDecodeError::input_too_large(
                    raw_input_bytes,
                    attempt
                        .input_budget()
                        .map_or(raw_input_bytes, ResourceBudget::limit),
                    privacy_policy,
                )
            })?;
        let input = std::str::from_utf8(input).map_err(|error| {
            NormalizingJsonDecodeError::invalid_utf8(
                error,
                raw_input_bytes,
                privacy_policy,
            )
        })?;
        let normalized = self
            .normalizer
            .normalize_after_raw_charge(input, &mut attempt)?;
        Self::admit_normalized_if_configured(
            self,
            &mut attempt,
            normalized.as_ref(),
            raw_input_bytes,
            privacy_policy,
        )?;
        let value = Self::deserialize_normalized(
            normalized.as_ref(),
            raw_input_bytes,
            normalized.len(),
            privacy_policy,
        )?;
        attempt.commit();
        Ok(value)
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
    /// # Type Parameters
    ///
    /// * `T` - Target type deserialized from the normalized JSON text.
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
    pub fn decode_slice<T>(
        &self,
        input: &[u8],
    ) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().diagnostic_policy();
        let mut session = self.decode_session();
        let mut attempt = session.begin_value();
        if attempt.try_consume_input_bytes(raw_input_bytes).is_err() {
            return Err(NormalizingJsonDecodeError::input_too_large(
                raw_input_bytes,
                attempt
                    .input_budget()
                    .map_or(raw_input_bytes, ResourceBudget::limit),
                privacy_policy,
            ));
        }
        let input = std::str::from_utf8(input).map_err(|error| {
            NormalizingJsonDecodeError::invalid_utf8(
                error,
                raw_input_bytes,
                privacy_policy,
            )
        })?;
        let normalized = self
            .normalizer
            .normalize_after_raw_charge(input, &mut attempt)?;
        Self::admit_normalized_if_configured(
            self,
            &mut attempt,
            normalized.as_ref(),
            raw_input_bytes,
            privacy_policy,
        )?;
        let value = Self::deserialize_normalized(
            normalized.as_ref(),
            raw_input_bytes,
            normalized.len(),
            privacy_policy,
        )?;
        attempt.commit();
        Ok(value)
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
    /// # Type Parameters
    ///
    /// * `T` - Target object type deserialized from the normalized JSON text.
    ///
    /// # Returns
    ///
    /// The deserialized target value when the normalized input is a JSON
    /// object.
    ///
    /// # Errors
    ///
    /// Returns [`NormalizingJsonDecodeError`] when normalization or parsing
    /// fails, when the top-level value is not an object, or when the object
    /// cannot be deserialized into `T`.
    ///
    /// # Panics
    ///
    /// Panics when the [`serde::Deserialize`] implementation for `T` panics.
    #[inline(always)]
    pub fn decode_object<T>(
        &self,
        input: &str,
    ) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        self.decode_with_top_level(input, JsonRootKind::Object)
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
    /// # Type Parameters
    ///
    /// * `T` - Element type deserialized from each array item.
    ///
    /// # Returns
    ///
    /// The deserialized elements when the normalized input is a JSON array.
    ///
    /// # Errors
    ///
    /// Returns [`NormalizingJsonDecodeError`] when normalization or parsing
    /// fails, when the top-level value is not an array, or when an element
    /// cannot be deserialized into `T`.
    ///
    /// # Panics
    ///
    /// Panics when the [`serde::Deserialize`] implementation for `T` panics.
    #[inline(always)]
    pub fn decode_array<T>(
        &self,
        input: &str,
    ) -> Result<Vec<T>, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        self.decode_with_top_level(input, JsonRootKind::Array)
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
    /// Returns [`NormalizingJsonDecodeError`] when input normalization or JSON
    /// parsing fails.
    pub fn decode_value(
        &self,
        input: &str,
    ) -> Result<Value, NormalizingJsonDecodeError> {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().diagnostic_policy();
        let mut session = self.decode_session();
        let mut attempt = session.begin_value();
        let normalized = self.normalizer.normalize(input, &mut attempt)?;
        Self::admit_normalized_if_configured(
            self,
            &mut attempt,
            normalized.as_ref(),
            raw_input_bytes,
            privacy_policy,
        )?;
        let value = Self::parse_value(
            normalized.as_ref(),
            raw_input_bytes,
            normalized.len(),
            privacy_policy,
        )?;
        attempt.commit();
        Ok(value)
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
    /// Returns [`NormalizingJsonDecodeError`] when normalization or parsing
    /// fails, when the validated top-level kind differs from `expected`, or
    /// when target deserialization fails.
    ///
    /// # Panics
    ///
    /// Panics from `T`'s `Deserialize` implementation or visitor methods are
    /// not caught and propagate to the caller.
    fn decode_with_top_level<T>(
        &self,
        input: &str,
        expected: JsonRootKind,
    ) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().diagnostic_policy();
        let mut session = self.decode_session();
        let mut attempt = session.begin_value();
        let normalized = self.normalizer.normalize(input, &mut attempt)?;
        let normalized_input_bytes = normalized.len();
        Self::admit_normalized_if_configured(
            self,
            &mut attempt,
            normalized.as_ref(),
            raw_input_bytes,
            privacy_policy,
        )?;
        let actual = JsonRootKind::of_normalized_json(normalized.as_ref());
        if actual != expected {
            Self::validate_json(
                normalized.as_ref(),
                raw_input_bytes,
                normalized_input_bytes,
                privacy_policy,
            )?;
            return Err(NormalizingJsonDecodeError::unexpected_top_level(
                expected,
                actual,
                raw_input_bytes,
                normalized_input_bytes,
                privacy_policy,
            ));
        }
        let value = Self::deserialize_normalized(
            normalized.as_ref(),
            raw_input_bytes,
            normalized_input_bytes,
            privacy_policy,
        )?;
        attempt.commit();
        Ok(value)
    }

    /// Creates the budget session for one lenient decode operation.
    fn decode_session(&self) -> JsonDecodeSession<'static> {
        let mut limits = JsonDecodeLimits::builder();
        if let Some(maximum) = self.options().max_input_bytes() {
            limits = limits.max_input_bytes(maximum);
        }
        if let Some(maximum) = self.options().max_normalized_bytes() {
            limits = limits.max_normalized_input_bytes(maximum);
        }
        if let Some(value_limits) = self.options().value_limits() {
            limits = limits.value_limits(value_limits);
        }
        JsonDecodeSession::owned(limits.build())
    }

    /// Runs lexical admission when value limits are configured.
    fn admit_normalized_if_configured(
        &self,
        attempt: &mut JsonDecodeAttempt<'_, JsonResource, usize>,
        normalized: &str,
        raw_input_bytes: usize,
        privacy_policy: DiagnosticPolicy,
    ) -> Result<(), NormalizingJsonDecodeError> {
        if self.options().value_limits().is_some() {
            JsonLexicalScanner::new(attempt.value_transaction_mut())
                .scan(normalized.as_bytes())
                .map_err(|error| {
                    Self::map_admission_error(
                        error,
                        normalized,
                        raw_input_bytes,
                        privacy_policy,
                    )
                })?;
        }
        Ok(())
    }

    /// Normalizes and directly deserializes input without value preflight.
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
    /// Returns [`NormalizingJsonDecodeError`] when normalization, JSON parsing,
    /// or target deserialization fails.
    fn normalize_then_deserialize<T>(
        &self,
        input: &str,
    ) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().diagnostic_policy();
        let mut session = self.decode_session();
        let mut attempt = session.begin_value();
        let normalized = self.normalizer.normalize(input, &mut attempt)?;
        Self::admit_normalized_if_configured(
            self,
            &mut attempt,
            normalized.as_ref(),
            raw_input_bytes,
            privacy_policy,
        )?;
        let value = Self::deserialize_normalized(
            normalized.as_ref(),
            raw_input_bytes,
            normalized.len(),
            privacy_policy,
        )?;
        attempt.commit();
        Ok(value)
    }

    /// Maps lexical admission failures to the stable public error model.
    ///
    /// # Parameters
    ///
    /// * `error` - Admission error produced while scanning normalized JSON.
    /// * `normalized` - Complete normalized JSON text.
    /// * `raw_input_bytes` - Input length before normalization.
    /// * `privacy_policy` - Policy applied to retained diagnostics.
    ///
    /// # Returns
    ///
    /// A budget error for resource rejection or an invalid-JSON error for a
    /// lexical failure, with input-derived details governed by
    /// `privacy_policy`.
    #[must_use]
    fn map_admission_error(
        error: JsonLexicalError<JsonResource, usize>,
        normalized: &str,
        raw_input_bytes: usize,
        privacy_policy: DiagnosticPolicy,
    ) -> NormalizingJsonDecodeError {
        let normalized_input_bytes = normalized.len();
        match error {
            JsonLexicalError::Budget(error) => {
                NormalizingJsonDecodeError::budget(
                    error,
                    raw_input_bytes,
                    normalized_input_bytes,
                    privacy_policy,
                )
            }
            JsonLexicalError::Syntax(error) => {
                match from_str::<&RawValue>(normalized) {
                    Ok(_) => NormalizingJsonDecodeError::invalid_lexical_json(
                        error,
                        raw_input_bytes,
                        normalized_input_bytes,
                        privacy_policy,
                    ),
                    Err(error) => NormalizingJsonDecodeError::invalid_json(
                        error,
                        raw_input_bytes,
                        normalized_input_bytes,
                        privacy_policy,
                    ),
                }
            }
        }
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
    /// Returns [`NormalizingJsonDecodeErrorKind::InvalidJson`](crate::decode::NormalizingJsonDecodeErrorKind::InvalidJson)
    /// when `normalized` is not valid JSON.
    #[inline]
    fn parse_value(
        normalized: &str,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: DiagnosticPolicy,
    ) -> Result<Value, NormalizingJsonDecodeError> {
        from_str(normalized).map_err(|error| {
            NormalizingJsonDecodeError::invalid_json(
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
    /// Returns [`NormalizingJsonDecodeErrorKind::InvalidJson`](crate::decode::NormalizingJsonDecodeErrorKind::InvalidJson)
    /// when validation fails.
    #[inline]
    fn validate_json(
        normalized: &str,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: DiagnosticPolicy,
    ) -> Result<(), NormalizingJsonDecodeError> {
        let _: &RawValue = from_str(normalized).map_err(|error| {
            NormalizingJsonDecodeError::invalid_json(
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
    /// Returns [`NormalizingJsonDecodeError`] classified as invalid JSON for
    /// syntax and end-of-input failures. A data error is classified as a
    /// deserialization failure only when complete syntax validation
    /// succeeds.
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
        privacy_policy: DiagnosticPolicy,
    ) -> Result<T, NormalizingJsonDecodeError>
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
        privacy_policy: DiagnosticPolicy,
    ) -> NormalizingJsonDecodeError {
        match error.classify() {
            Category::Data => match Self::validate_json(
                normalized,
                raw_input_bytes,
                normalized_input_bytes,
                privacy_policy,
            ) {
                Ok(()) => NormalizingJsonDecodeError::deserialize(
                    error,
                    raw_input_bytes,
                    normalized_input_bytes,
                    privacy_policy,
                ),
                Err(error) => error,
            },
            Category::Io | Category::Syntax | Category::Eof => {
                NormalizingJsonDecodeError::invalid_json(
                    error,
                    raw_input_bytes,
                    normalized_input_bytes,
                    privacy_policy,
                )
            }
        }
    }
}

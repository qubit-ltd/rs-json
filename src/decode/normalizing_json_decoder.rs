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
#[derive(Debug)]
pub struct NormalizingJsonDecoder<'budget> {
    /// Stores the configured normalization pipeline.
    normalizer: JsonNormalizer,
    /// Session that accumulates all input and value charges.
    session: JsonDecodeSession<'budget>,
}

impl Default for NormalizingJsonDecoder<'static> {
    fn default() -> Self {
        Self::new(NormalizingJsonDecodeOptions::default())
    }
}

impl<'budget> NormalizingJsonDecoder<'budget> {
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
    pub fn new(options: NormalizingJsonDecodeOptions) -> Self {
        let session = Self::session_from_options(&options);
        Self {
            normalizer: JsonNormalizer::new(options),
            session,
        }
    }

    /// Creates an owned session from the resource limits in `options`.
    fn session_from_options(
        options: &NormalizingJsonDecodeOptions,
    ) -> JsonDecodeSession<'static> {
        let mut limits = JsonDecodeLimits::builder();
        if let Some(maximum) = options.max_input_bytes() {
            limits = limits.max_input_bytes(maximum);
        }
        if let Some(maximum) = options.max_normalized_bytes() {
            limits = limits.max_normalized_input_bytes(maximum);
        }
        if let Some(value_limits) = options.value_limits() {
            limits = limits.value_limits(value_limits);
        }
        JsonDecodeSession::owned(limits.build())
    }
}

impl<'budget> NormalizingJsonDecoder<'budget> {
    /// Creates a decoder with a caller-provided cumulative session.
    ///
    /// The supplied session is the only accounting state and limit source;
    /// resource-limit fields in `options` are not merged into it.
    #[must_use]
    pub fn with_session(
        options: NormalizingJsonDecodeOptions,
        session: JsonDecodeSession<'budget>,
    ) -> Self {
        Self {
            normalizer: JsonNormalizer::new(options),
            session,
        }
    }

    /// Returns the cumulative session for read-only inspection.
    #[must_use]
    pub const fn session(&self) -> &JsonDecodeSession<'budget> {
        &self.session
    }

    /// Returns mutable access to the cumulative session.
    #[must_use]
    pub const fn session_mut(&mut self) -> &mut JsonDecodeSession<'budget> {
        &mut self.session
    }

    /// Returns the cumulative session and consumes the decoder.
    #[must_use]
    pub fn into_session(self) -> JsonDecodeSession<'budget> {
        self.session
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
    pub fn decode_str<T>(
        &mut self,
        input: &str,
    ) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        self.normalize_then_deserialize(input)
    }

    /// Decodes one UTF-8 byte slice while accumulating charges in this decoder.
    pub fn decode_utf8<T>(
        &mut self,
        input: &[u8],
    ) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().diagnostic_policy();
        let mut attempt = self.session.begin_value();
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

    /// The configured raw byte limit is enforced before UTF-8 validation.
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
        &mut self,
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
        &mut self,
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
        &mut self,
        input: &str,
    ) -> Result<Value, NormalizingJsonDecodeError> {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().diagnostic_policy();
        let mut attempt = self.session.begin_value();
        let normalized = self.normalizer.normalize(input, &mut attempt)?;
        Self::admit_normalized_if_configured(
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
        &mut self,
        input: &str,
        expected: JsonRootKind,
    ) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().diagnostic_policy();
        let mut attempt = self.session.begin_value();
        let normalized = self.normalizer.normalize(input, &mut attempt)?;
        let normalized_input_bytes = normalized.len();
        Self::admit_normalized_if_configured(
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

    /// Runs lexical admission against this decoder's session.
    fn admit_normalized_if_configured(
        attempt: &mut JsonDecodeAttempt<'_, JsonResource, usize>,
        normalized: &str,
        raw_input_bytes: usize,
        privacy_policy: DiagnosticPolicy,
    ) -> Result<(), NormalizingJsonDecodeError> {
        JsonLexicalScanner::new(attempt.value_transaction_mut())
            .scan(normalized.as_bytes())
            .map_err(|error| {
                Self::map_admission_error(
                    error,
                    normalized,
                    raw_input_bytes,
                    privacy_policy,
                )
            })
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
        &mut self,
        input: &str,
    ) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let raw_input_bytes = input.len();
        let privacy_policy = self.options().diagnostic_policy();
        let mut attempt = self.session.begin_value();
        let normalized = self.normalizer.normalize(input, &mut attempt)?;
        Self::admit_normalized_if_configured(
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

#[allow(missing_docs)]
impl<'budget> NormalizingJsonDecoder<'budget> {
    #[deprecated(note = "use with_session(...).decode_str(...)")]
    pub fn decode_with_session<T>(
        &self,
        input: &str,
        session: &mut JsonDecodeSession<'budget>,
    ) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    {
        let mut decoder = Self::with_session(self.options().clone(),
            std::mem::replace(session, JsonDecodeSession::owned(JsonDecodeLimits::default())));
        let result = decoder.decode_str(input);
        *session = decoder.into_session();
        result
    }

    #[deprecated(note = "use decode_utf8(...) on a stateful decoder")]
    pub fn decode_slice<T>(&self, input: &[u8]) -> Result<T, NormalizingJsonDecodeError>
    where
        T: DeserializeOwned,
    { let mut decoder = Self::new(self.options().clone()); decoder.decode_utf8(input) }
}

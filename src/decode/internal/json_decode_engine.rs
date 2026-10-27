// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared resource admission and materialization for JSON decoder facades.

use std::error::Error;
use std::io;
use std::sync::Arc;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonDecodeAttempt;
use qubit_budget::json::JsonDecodeSession;
use qubit_io::Input;
use serde::de::DeserializeSeed;
use serde::de::Error as _;
use serde_json::from_slice;
use serde_json::value::RawValue;

use super::DecodeMetadata;
use super::JsonNormalizer;
use super::admit_json_document;
use super::deserialize_json_document;
use super::json_document_decoder::admit_json_document_with_number_model;
use super::validated_consumer_error::ValidatedConsumerError;
use super::validated_value_consumer::ValidatedValueConsumer;
use crate::decode::DiagnosticPolicy;
#[cfg(feature = "jcs")]
use crate::decode::JcsDecodeError;
use crate::decode::JsonDecodeContext;
use crate::decode::JsonDecodeError;
use crate::decode::JsonDecodeNumberModel;
use crate::decode::JsonDecodeStage;
use crate::decode::JsonMaterializationErrorReason;
use crate::decode::JsonNumberPolicy;
use crate::decode::JsonNumberToken;
use crate::decode::JsonPathSegment;
use crate::decode::JsonRootKind;
use crate::decode::JsonValidatedDecodeError;
use crate::decode::JsonValidatedDecodeOptions;
use crate::decode::JsonValidatedLimit;
use crate::decode::NormalizedJsonDocument;
use crate::lexical::JsonLexicalError;
use crate::lexical::JsonLexicalErrorReason;
use crate::lexical::JsonLexicalScanError;
use crate::lexical::JsonLexicalScanner;
use crate::lexical::JsonNumberModel;
#[cfg(feature = "jcs")]
use crate::lexical::line_column_at;
use crate::value::SafeJsonValue;
use crate::value::internal::JsonValueMaterializationError;
use crate::value::internal::LexicalJsonValueBuilder;

/// Shared generic execution core used by both public decoder facades.
///
/// # Type Parameters
///
/// * `'budget` - Lifetime of any caller-owned budget borrowed by the session.
/// * `R` - Resource identity tracked by the decoder session.
/// * `Q` - Quantity representation used for resource accounting.
#[derive(Debug)]
pub(in crate::decode) struct JsonDecodeEngine<'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Cumulative caller-owned or owned accounting state.
    session: JsonDecodeSession<'budget, R, Q>,
}

impl<'budget, R, Q> JsonDecodeEngine<'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates an engine around one reusable decode session.
    ///
    /// # Parameters
    ///
    /// * `session` - Accounting session reused by subsequent decode operations.
    ///
    /// # Returns
    ///
    /// An engine that owns the supplied session.
    #[inline]
    #[must_use]
    pub(in crate::decode) const fn new(session: JsonDecodeSession<'budget, R, Q>) -> Self {
        Self { session }
    }

    /// Returns the cumulative session for read-only inspection.
    ///
    /// # Returns
    ///
    /// A shared reference to the session and its cumulative accounting state.
    #[inline(always)]
    #[must_use]
    pub(in crate::decode) const fn session(&self) -> &JsonDecodeSession<'budget, R, Q> {
        &self.session
    }

    /// Returns mutable access to the cumulative session.
    ///
    /// # Returns
    ///
    /// A mutable reference to the session used by future decode operations.
    #[inline(always)]
    #[must_use]
    pub(in crate::decode) const fn session_mut(&mut self) -> &mut JsonDecodeSession<'budget, R, Q> {
        &mut self.session
    }

    /// Consumes the engine and returns its cumulative session.
    ///
    /// # Returns
    ///
    /// The session previously owned by the engine.
    #[inline(always)]
    #[must_use]
    pub(in crate::decode) fn into_session(self) -> JsonDecodeSession<'budget, R, Q> {
        self.session
    }

    /// Prepares one string with immediate raw and normalized input accounting.
    ///
    /// # Parameters
    ///
    /// * `normalizer` - Policy and normalization pipeline for the input.
    /// * `input` - UTF-8 JSON-like text to normalize and account.
    ///
    /// # Returns
    ///
    /// A normalized document that retains both raw and normalized lengths.
    ///
    /// # Errors
    ///
    /// Returns a normalization or budget error when processing is rejected.
    pub(in crate::decode) fn prepare_str<'input>(
        &mut self,
        normalizer: &JsonNormalizer,
        input: &'input str,
    ) -> Result<NormalizedJsonDocument<'input>, JsonDecodeError<R, Q>> {
        let raw_input_bytes = input.len();
        let diagnostic_policy = normalizer.policy().diagnostic_policy();
        let mut attempt = self.session.begin_value();
        let normalized = normalizer.normalize(input, &mut attempt)?;
        let document = NormalizedJsonDocument::new(normalized, raw_input_bytes);
        debug_assert_eq!(document.raw_input_bytes(), raw_input_bytes);
        debug_assert_eq!(diagnostic_policy, normalizer.policy().diagnostic_policy());
        Ok(document)
    }

    /// Prepares one byte slice after charging raw input and validating UTF-8.
    ///
    /// # Parameters
    ///
    /// * `normalizer` - Policy and normalization pipeline for the input.
    /// * `input` - Raw bytes that must be charged and decoded as UTF-8.
    ///
    /// # Returns
    ///
    /// A normalized document retaining the original byte count.
    ///
    /// # Errors
    ///
    /// Returns an input-budget, UTF-8, normalization, or value-budget error.
    pub(in crate::decode) fn prepare_utf8<'input>(
        &mut self,
        normalizer: &JsonNormalizer,
        input: &'input [u8],
    ) -> Result<NormalizedJsonDocument<'input>, JsonDecodeError<R, Q>> {
        let raw_input_bytes = input.len();
        let diagnostic_policy = normalizer.policy().diagnostic_policy();
        let mut attempt = self.session.begin_value();
        attempt.try_consume_input_bytes(raw_input_bytes).map_err(|source| {
            JsonDecodeError::budget(source, JsonDecodeStage::Input, raw_input_bytes, None, diagnostic_policy)
        })?;
        let input = std::str::from_utf8(input)
            .map_err(|source| JsonDecodeError::invalid_utf8(source, raw_input_bytes, diagnostic_policy))?;
        let normalized = normalizer.normalize_after_raw_charge(input, &mut attempt)?;
        Ok(NormalizedJsonDocument::new(normalized, raw_input_bytes))
    }

    /// Strictly validates and deserializes one complete byte document.
    ///
    /// # Type Parameters
    ///
    /// * `S` - Serde seed that constructs the requested result.
    ///
    /// # Parameters
    ///
    /// * `seed` - Deserializer seed for the result value.
    /// * `input` - Complete JSON document bytes.
    /// * `diagnostic_policy` - Policy controlling retained source details.
    ///
    /// # Returns
    ///
    /// The value produced by `seed` after strict admission succeeds.
    ///
    /// # Errors
    ///
    /// Returns an input, UTF-8, syntax, budget, or Serde deserialization error.
    pub(in crate::decode) fn decode_seed_utf8<'de, S>(
        &mut self,
        seed: S,
        input: &'de [u8],
        diagnostic_policy: DiagnosticPolicy,
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        self.decode_seed_utf8_with_number_model(seed, input, diagnostic_policy, JsonNumberModel::Integer64)
    }

    /// Returns one-based Serde-compatible coordinates for a materialization
    /// token.
    ///
    /// Counts bytes before the token offset; only LF starts a new line.
    fn materialization_error_position(input: &[u8], offset: usize) -> (usize, usize) {
        let mut line = 1;
        let mut column = 1;
        for byte in &input[..offset.min(input.len())] {
            if *byte == b'\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        (line, column)
    }

    /// Decodes a validated-policy document from an in-memory byte slice.
    ///
    /// `charge_input` must be false when these bytes were already charged while
    /// reading an input stream. The policy receives syntactically validated
    /// number tokens and their tracked paths; the selected numeric range is
    /// checked afterward. This method commits value measurements only after
    /// scanning, materialization, and the optional root
    /// check all succeed.
    ///
    /// # Errors
    ///
    /// Returns an input-budget, UTF-8, lexical/value-limit, policy, exact-safe-
    /// integer, materialization, root-kind, or commit error. A failed attempt
    /// does not commit its staged value measurements.
    pub(in crate::decode) fn decode_unique_validated_buffer<E, F>(
        &mut self,
        input: &[u8],
        options: &JsonValidatedDecodeOptions,
        policy: F,
        diagnostic_policy: DiagnosticPolicy,
        charge_input: bool,
    ) -> Result<SafeJsonValue, JsonValidatedDecodeError<E, R, Q>>
    where
        F: for<'call> FnMut(&'call [JsonPathSegment], JsonNumberToken<'call>) -> Result<JsonNumberPolicy, E>,
    {
        self.decode_unique_value_buffer(input, options, policy, diagnostic_policy, charge_input, true)
    }

    /// Shared transactional scanner and materialization path for owned values.
    ///
    /// `charge_input` controls whether this call charges the original input
    /// bytes; stream readers pass false after charging during the read.
    /// `track_path` controls whether the consumer maintains path segments for
    /// policy callbacks. Policy callbacks receive number tokens whose JSON
    /// syntax has been validated; the selected numeric range is checked after
    /// the callback returns.
    ///
    /// # Errors
    ///
    /// Returns an input-budget, UTF-8, lexical/value-limit, policy,
    /// materialization, root-kind, or commit error. The value transaction is
    /// committed only after all checks succeed.
    fn decode_unique_value_buffer<E, F>(
        &mut self,
        input: &[u8],
        options: &JsonValidatedDecodeOptions,
        policy: F,
        diagnostic_policy: DiagnosticPolicy,
        charge_input: bool,
        track_path: bool,
    ) -> Result<SafeJsonValue, JsonValidatedDecodeError<E, R, Q>>
    where
        F: for<'call> FnMut(&'call [JsonPathSegment], JsonNumberToken<'call>) -> Result<JsonNumberPolicy, E>,
    {
        let raw_input_bytes = input.len();
        let depth_limit = self.session.value_budget().limits().max_depth();
        let has_value_limits = self.session.value_budget().limits().has_limits();
        let mut attempt = self.session.begin_value();
        if charge_input {
            attempt
                .try_consume_input_bytes(raw_input_bytes)
                .map_err(|source| JsonValidatedDecodeError::Decode {
                    error: JsonDecodeError::budget(
                        source,
                        JsonDecodeStage::Input,
                        raw_input_bytes,
                        None,
                        diagnostic_policy,
                    ),
                    context: None,
                })?;
        }
        std::str::from_utf8(input).map_err(|source| JsonValidatedDecodeError::Decode {
            error: JsonDecodeError::invalid_utf8(source, raw_input_bytes, diagnostic_policy),
            context: None,
        })?;
        let model = match options.number_model() {
            JsonDecodeNumberModel::Integer64 => JsonNumberModel::Integer64,
            JsonDecodeNumberModel::Integer64WithFiniteFallback => JsonNumberModel::Integer64WithFiniteFallback,
        };
        let mut consumer = ValidatedValueConsumer::new(options.clone(), policy, model, track_path);
        let result = JsonLexicalScanner::new(attempt.value_transaction_mut(), has_value_limits)
            .with_number_model(model)
            .with_depth_limit(depth_limit)
            .with_max_encoded_string_bytes(options.max_encoded_string_bytes())
            .scan_with_consumer(input, &mut consumer);
        match result {
            Ok(()) => {}
            Err(JsonLexicalScanError::Consumer(error)) => {
                return Err(match error {
                    ValidatedConsumerError::Policy { source, offset } => {
                        JsonValidatedDecodeError::Policy { source, offset }
                    }
                    ValidatedConsumerError::Exact { source, offset } => {
                        JsonValidatedDecodeError::ExactSafeInteger { source, offset }
                    }
                    ValidatedConsumerError::Limit {
                        limit,
                        maximum,
                        observed,
                        offset,
                    } => JsonValidatedDecodeError::Limit {
                        limit,
                        maximum,
                        observed,
                        offset,
                    },
                    ValidatedConsumerError::Materialization { source, context } => {
                        let (reason, message, offset) = match source {
                            JsonValueMaterializationError::DuplicateKey { offset } => (
                                JsonMaterializationErrorReason::DuplicateKey,
                                "duplicate JSON object key",
                                offset,
                            ),
                            JsonValueMaterializationError::InvalidNumber { offset } => (
                                JsonMaterializationErrorReason::InvalidNumber,
                                "invalid strict JSON number",
                                offset,
                            ),
                        };
                        let (line, column) = Self::materialization_error_position(input, offset);
                        let source =
                            serde_json::Error::custom(format_args!("{message} at line {line} column {column}"));
                        JsonValidatedDecodeError::Decode {
                            error: JsonDecodeError::materialization(source, reason, raw_input_bytes, diagnostic_policy),
                            context: Some(context),
                        }
                    }
                });
            }
            Err(JsonLexicalScanError::EncodedStringLimit {
                maximum,
                observed,
                offset,
            }) => {
                return Err(JsonValidatedDecodeError::Limit {
                    limit: JsonValidatedLimit::EncodedStringBytes,
                    maximum,
                    observed,
                    offset,
                });
            }
            Err(JsonLexicalScanError::Lexical(error)) => {
                let context = match &error {
                    JsonLexicalError::Syntax(failure) => Some(match failure.reason {
                        JsonLexicalErrorReason::ExpectedColon => JsonDecodeContext::Colon,
                        JsonLexicalErrorReason::ExpectedObjectKey => JsonDecodeContext::ObjectKey,
                        JsonLexicalErrorReason::ExpectedCommaOrArrayEnd => JsonDecodeContext::ArraySeparator,
                        JsonLexicalErrorReason::ExpectedCommaOrObjectEnd => JsonDecodeContext::ObjectSeparator,
                        JsonLexicalErrorReason::TrailingCharacters => JsonDecodeContext::TrailingInput,
                        JsonLexicalErrorReason::EncodedStringLimit { .. } => JsonDecodeContext::String,
                        JsonLexicalErrorReason::InvalidNumber
                        | JsonLexicalErrorReason::IntegerOutOfRange
                        | JsonLexicalErrorReason::FloatOutOfRange => JsonDecodeContext::Number,
                        _ => JsonDecodeContext::Value,
                    }),
                    JsonLexicalError::Budget(_) => None,
                };
                return Err(JsonValidatedDecodeError::Decode {
                    error: Self::map_validated_lexical_error(error, raw_input_bytes, diagnostic_policy),
                    context,
                });
            }
        }
        let value = consumer.finish().map_err(|source| {
            let reason = match source {
                JsonValueMaterializationError::DuplicateKey { .. } => JsonMaterializationErrorReason::DuplicateKey,
                JsonValueMaterializationError::InvalidNumber { .. } => JsonMaterializationErrorReason::InvalidNumber,
            };
            let (line, column) = Self::materialization_error_position(input, 0);
            let source =
                serde_json::Error::custom(format_args!("materialization failed at line {line} column {column}"));
            JsonValidatedDecodeError::Decode {
                error: JsonDecodeError::materialization(source, reason, raw_input_bytes, diagnostic_policy),
                context: Some(JsonDecodeContext::Value),
            }
        })?;
        if let Some(expected) = options.root_kind() {
            let actual = JsonRootKind::of(value.as_value());
            if expected != actual {
                return Err(JsonValidatedDecodeError::Decode {
                    error: JsonDecodeError::unexpected_top_level(
                        expected,
                        actual,
                        raw_input_bytes,
                        None,
                        diagnostic_policy,
                    ),
                    context: None,
                });
            }
        }
        attempt.commit().map_err(|source| JsonValidatedDecodeError::Decode {
            error: JsonDecodeError::budget(
                source,
                JsonDecodeStage::Admission,
                raw_input_bytes,
                None,
                diagnostic_policy,
            ),
            context: None,
        })?;
        Ok(value)
    }

    /// Decodes one complete input slice into a protected owned JSON value.
    ///
    /// Input bytes are charged by this call. The private buffer helper can skip
    /// that charge only when an input-reading path has already charged them.
    ///
    /// # Errors
    ///
    /// Returns an input-budget, UTF-8, lexical, value-budget, materialization,
    /// or commit error. Value measurements are committed only on success.
    pub(in crate::decode) fn decode_value_utf8(
        &mut self,
        input: &[u8],
        diagnostic_policy: DiagnosticPolicy,
    ) -> Result<SafeJsonValue, JsonDecodeError<R, Q>> {
        self.decode_owned_value_buffer(input, diagnostic_policy, true)
    }

    /// Decodes an already-buffered owned value, optionally charging its input.
    ///
    /// This path disables path tracking because it does not invoke an external
    /// number policy. Callers reading a stream pass `charge_input = false`
    /// after the read path has charged those bytes.
    fn decode_owned_value_buffer(
        &mut self,
        input: &[u8],
        diagnostic_policy: DiagnosticPolicy,
        charge_input: bool,
    ) -> Result<SafeJsonValue, JsonDecodeError<R, Q>> {
        let options = JsonValidatedDecodeOptions::builder().build();
        self.decode_unique_value_buffer(
            input,
            &options,
            |_, _| Ok::<_, std::convert::Infallible>(JsonNumberPolicy::Preserve),
            diagnostic_policy,
            charge_input,
            false,
        )
        .map_err(|error| match error {
            JsonValidatedDecodeError::Decode { error, .. } => error,
            JsonValidatedDecodeError::Policy { source, .. } => match source {},
            JsonValidatedDecodeError::ExactSafeInteger { .. } | JsonValidatedDecodeError::Limit { .. } => {
                unreachable!("owned value decoding has no policy or additional limits")
            }
        })
    }

    /// Decodes a validated-policy document from a byte slice and charges its
    /// input bytes once.
    ///
    /// The policy receives syntactically validated number tokens and their JSON
    /// paths; the selected numeric range is checked afterward.
    ///
    /// # Errors
    ///
    /// Returns the same admission, policy, materialization, root-kind, and
    /// commit errors as the buffer helper.
    pub(in crate::decode) fn decode_unique_validated_utf8<E, F>(
        &mut self,
        input: &[u8],
        options: &JsonValidatedDecodeOptions,
        policy: F,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Result<SafeJsonValue, JsonValidatedDecodeError<E, R, Q>>
    where
        F: for<'call> FnMut(&'call [JsonPathSegment], JsonNumberToken<'call>) -> Result<JsonNumberPolicy, E>,
    {
        self.decode_unique_validated_buffer(input, options, policy, diagnostic_policy, true)
    }

    /// Reads, charges, then decodes one validated-policy input document.
    ///
    /// The buffer decoder is told not to charge the bytes again. The policy
    /// receives syntactically validated number tokens and their JSON paths;
    /// the selected numeric range is checked afterward.
    ///
    /// # Errors
    ///
    /// Returns an input read/budget error or any validated decoding error.
    pub(in crate::decode) fn decode_unique_validated_input<E, F, I>(
        &mut self,
        input: &mut I,
        options: &JsonValidatedDecodeOptions,
        policy: F,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Result<SafeJsonValue, JsonValidatedDecodeError<E, R, Q>>
    where
        I: qubit_io::ByteInput + ?Sized,
        F: for<'call> FnMut(&'call [JsonPathSegment], JsonNumberToken<'call>) -> Result<JsonNumberPolicy, E>,
    {
        let bytes = {
            let mut attempt = self.session.begin_value();
            Self::read_input_bytes(&mut attempt, input, diagnostic_policy)
                .map_err(|error| JsonValidatedDecodeError::Decode { error, context: None })?
        };
        self.decode_unique_validated_buffer(&bytes, options, policy, diagnostic_policy, false)
    }

    /// Reads and charges one input document before decoding it as an owned
    /// protected value; the buffered decode does not charge those bytes again.
    ///
    /// # Errors
    ///
    /// Returns an input read/budget error or any owned-value decoding error.
    pub(in crate::decode) fn decode_value_input<I>(
        &mut self,
        input: &mut I,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Result<SafeJsonValue, JsonDecodeError<R, Q>>
    where
        I: qubit_io::ByteInput + ?Sized,
    {
        let bytes = {
            let mut attempt = self.session.begin_value();
            Self::read_input_bytes(&mut attempt, input, diagnostic_policy)?
        };
        self.decode_owned_value_buffer(&bytes, diagnostic_policy, false)
    }

    /// Admits a unique-key document and deserializes native events atomically.
    ///
    /// Returns a decode error for admission, materialization, or seed failure;
    /// input charges remain consumed while staged value charges roll back.
    pub(in crate::decode) fn decode_unique_seed_utf8<S, T>(
        &mut self,
        seed: S,
        input: &[u8],
        diagnostic_policy: DiagnosticPolicy,
    ) -> Result<T, JsonDecodeError<R, Q>>
    where
        S: for<'de> DeserializeSeed<'de, Value = T>,
    {
        let metadata = DecodeMetadata {
            raw_input_bytes: input.len(),
            normalized_input_bytes: None,
            diagnostic_policy,
            number_model: JsonNumberModel::Integer64,
        };
        let has_value_limits = self.session.value_budget().limits().has_limits();
        let mut attempt = self.session.begin_value();
        Self::prepare_attempt(&mut attempt, input, metadata, true, has_value_limits)?;
        let mut builder = LexicalJsonValueBuilder::new(JsonNumberModel::Integer64);
        JsonLexicalScanner::new(attempt.value_transaction_mut(), false)
            .scan_with_events(input, |event| {
                let _ = builder.try_accept(event);
            })
            .map_err(|error| Self::map_lexical_error(error, input, metadata))?;
        let tree = builder.finish_guarded().map_err(|error| {
            let (reason, message, offset) = match error {
                JsonValueMaterializationError::DuplicateKey { offset } => (
                    JsonMaterializationErrorReason::DuplicateKey,
                    "duplicate JSON object key",
                    offset,
                ),
                JsonValueMaterializationError::InvalidNumber { offset } => (
                    JsonMaterializationErrorReason::InvalidNumber,
                    "invalid strict JSON number",
                    offset,
                ),
            };
            let (line, column) = Self::materialization_error_position(input, offset);
            let error = serde_json::Error::custom(format_args!("{message} at line {line} column {column}"));
            JsonDecodeError::materialization(error, reason, input.len(), diagnostic_policy)
        })?;
        // The builder constructs canonical i64/u64/f64 Numbers. Unlike direct
        // arbitrary-precision text parsing, Value deserialization exposes these
        // as native numeric events, without guessing map-key provenance.
        let value = seed
            .deserialize(tree.as_value())
            .map_err(|error| JsonDecodeError::deserialize(error, input.len(), None, diagnostic_policy))?;
        attempt.commit().map_err(|error| {
            JsonDecodeError::budget(error, JsonDecodeStage::Admission, input.len(), None, diagnostic_policy)
        })?;
        Ok(value)
    }

    /// Decodes a complete document with an explicit internal number contract.
    ///
    /// # Type Parameters
    ///
    /// * `S` - Serde seed that constructs the requested result.
    ///
    /// # Parameters
    ///
    /// * `seed` - Deserializer seed for the result value.
    /// * `input` - Complete JSON document bytes.
    /// * `diagnostic_policy` - Policy controlling retained source details.
    /// * `number_model` - Numeric representation contract applied at admission.
    ///
    /// # Returns
    ///
    /// The value produced by `seed` after admission succeeds.
    ///
    /// # Errors
    ///
    /// Returns an input, UTF-8, syntax, budget, or Serde deserialization error.
    pub(in crate::decode) fn decode_seed_utf8_with_number_model<'de, S>(
        &mut self,
        seed: S,
        input: &'de [u8],
        diagnostic_policy: DiagnosticPolicy,
        number_model: JsonNumberModel,
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        let metadata = DecodeMetadata {
            raw_input_bytes: input.len(),
            normalized_input_bytes: None,
            diagnostic_policy,
            number_model,
        };
        self.decode_seed(seed, input, metadata, true, None)
    }

    /// Strictly validates and deserializes one complete byte document while
    /// enforcing an optional top-level kind.
    ///
    /// # Type Parameters
    ///
    /// * `S` - Serde seed that constructs the requested result.
    ///
    /// # Parameters
    ///
    /// * `seed` - Deserializer seed for the result value.
    /// * `input` - Complete JSON document bytes.
    /// * `diagnostic_policy` - Policy controlling retained source details.
    /// * `expected` - Optional required root object or array kind.
    ///
    /// # Returns
    ///
    /// The value produced by `seed` after document and root-kind checks.
    ///
    /// # Errors
    ///
    /// Returns an input, UTF-8, syntax, budget, root-kind, or Serde error.
    pub(in crate::decode) fn decode_seed_utf8_with_top_level<'de, S>(
        &mut self,
        seed: S,
        input: &'de [u8],
        diagnostic_policy: DiagnosticPolicy,
        expected: Option<JsonRootKind>,
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        let metadata = DecodeMetadata {
            raw_input_bytes: input.len(),
            normalized_input_bytes: None,
            diagnostic_policy,
            number_model: JsonNumberModel::Integer64,
        };
        self.decode_seed(seed, input, metadata, true, expected)
    }

    /// Deserializes one prepared document without charging its input again.
    ///
    /// # Type Parameters
    ///
    /// * `S` - Serde seed that constructs the requested result.
    ///
    /// # Parameters
    ///
    /// * `document` - Previously normalized and charged document.
    /// * `seed` - Deserializer seed for the result value.
    /// * `diagnostic_policy` - Policy controlling retained source details.
    /// * `expected` - Optional required root object or array kind.
    ///
    /// # Returns
    ///
    /// The value produced by `seed` after the prepared document is admitted.
    ///
    /// # Errors
    ///
    /// Returns a syntax, budget, root-kind, or Serde deserialization error.
    pub(in crate::decode) fn decode_document_seed<'de, S>(
        &mut self,
        document: &'de NormalizedJsonDocument<'_>,
        seed: S,
        diagnostic_policy: DiagnosticPolicy,
        expected: Option<JsonRootKind>,
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        let metadata = DecodeMetadata {
            raw_input_bytes: document.raw_input_bytes(),
            normalized_input_bytes: Some(document.normalized_input_bytes()),
            diagnostic_policy,
            number_model: JsonNumberModel::Integer64,
        };
        self.decode_seed(seed, document.as_str().as_bytes(), metadata, false, expected)
    }

    /// Strictly validates one complete byte document and commits value usage.
    ///
    /// # Parameters
    ///
    /// * `input` - Complete UTF-8 JSON document bytes.
    /// * `diagnostic_policy` - Policy controlling retained source details.
    ///
    /// # Returns
    ///
    /// `Ok(())` after strict admission and accounting commit.
    ///
    /// # Errors
    ///
    /// Returns an input, UTF-8, syntax, or budget error.
    pub(in crate::decode) fn validate_utf8(
        &mut self,
        input: &[u8],
        diagnostic_policy: DiagnosticPolicy,
    ) -> Result<(), JsonDecodeError<R, Q>> {
        let metadata = DecodeMetadata {
            raw_input_bytes: input.len(),
            normalized_input_bytes: None,
            diagnostic_policy,
            number_model: JsonNumberModel::Integer64,
        };
        self.validate(input, metadata, true, None)
    }

    /// Validates one prepared document without charging its input again.
    ///
    /// # Parameters
    ///
    /// * `document` - Previously normalized and charged document.
    /// * `diagnostic_policy` - Policy controlling retained source details.
    /// * `expected` - Optional required root object or array kind.
    ///
    /// # Returns
    ///
    /// `Ok(())` after admission and optional root-kind checks succeed.
    ///
    /// # Errors
    ///
    /// Returns a syntax, budget, or root-kind error.
    pub(in crate::decode) fn validate_document(
        &mut self,
        document: &NormalizedJsonDocument<'_>,
        diagnostic_policy: DiagnosticPolicy,
        expected: Option<JsonRootKind>,
    ) -> Result<(), JsonDecodeError<R, Q>> {
        let metadata = DecodeMetadata {
            raw_input_bytes: document.raw_input_bytes(),
            normalized_input_bytes: Some(document.normalized_input_bytes()),
            diagnostic_policy,
            number_model: JsonNumberModel::Integer64,
        };
        self.validate(document.as_str().as_bytes(), metadata, false, expected)
    }

    /// Reads and accounts one complete input before applying the shared strict
    /// document decoder. Bytes are buffered because returned values are owned.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Owned value produced from the input document.
    /// * `I` - Byte input source read through EOF.
    ///
    /// # Parameters
    ///
    /// * `input` - Source of the complete JSON document.
    /// * `diagnostic_policy` - Policy controlling retained error details.
    ///
    /// # Returns
    ///
    /// The owned value deserialized from the input.
    ///
    /// # Errors
    ///
    /// Returns a read, input-budget, UTF-8, syntax, budget, or deserialization
    /// error.
    pub(in crate::decode) fn decode_input<T, I>(
        &mut self,
        input: &mut I,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: serde::de::DeserializeOwned,
        I: Input<Item = u8> + ?Sized,
    {
        self.decode_input_seed(
            crate::decode::internal::TypedSeed::<T>::new(),
            input,
            diagnostic_policy,
            JsonNumberModel::Integer64,
        )
    }

    /// Reads an owned value through a supplied seed and one selected number
    /// contract.
    ///
    /// # Type Parameters
    ///
    /// * `S` - Seed that constructs the owned result value.
    /// * `T` - Owned result type produced by `S`.
    /// * `I` - Byte input source read through EOF.
    ///
    /// # Parameters
    ///
    /// * `seed` - Deserializer seed for the result value.
    /// * `input` - Source of the complete JSON document.
    /// * `diagnostic_policy` - Policy controlling retained error details.
    /// * `number_model` - Numeric representation contract applied at admission.
    ///
    /// # Returns
    ///
    /// The owned value produced by `seed`.
    ///
    /// # Errors
    ///
    /// Returns a read, input-budget, UTF-8, syntax, budget, or seed error.
    pub(in crate::decode) fn decode_input_seed<S, T, I>(
        &mut self,
        seed: S,
        input: &mut I,
        diagnostic_policy: DiagnosticPolicy,
        number_model: JsonNumberModel,
    ) -> Result<T, JsonDecodeError<R, Q>>
    where
        S: for<'de> DeserializeSeed<'de, Value = T>,
        I: Input<Item = u8> + ?Sized,
    {
        let has_value_limits = self.session.value_budget().limits().has_limits();
        let mut attempt = self.session.begin_value();
        let bytes = Self::read_input_bytes(&mut attempt, input, diagnostic_policy)?;
        let raw_input_bytes = bytes.len();
        let metadata = DecodeMetadata {
            raw_input_bytes,
            normalized_input_bytes: None,
            diagnostic_policy,
            number_model,
        };
        let value = Self::decode_seed_attempt(&mut attempt, seed, &bytes, metadata, false, has_value_limits, None)?;
        attempt.commit().map_err(|source| {
            JsonDecodeError::budget(
                source,
                JsonDecodeStage::Admission,
                metadata.raw_input_bytes,
                None,
                diagnostic_policy,
            )
        })?;
        Ok(value)
    }

    /// Decodes a complete UTF-8 JSON document into an owned JCS value.
    ///
    /// # Parameters
    ///
    /// * `input` - Complete JSON document bytes.
    /// * `diagnostic_policy` - Policy for retaining input-derived error detail.
    ///
    /// # Returns
    ///
    /// The materialized JSON value using the JCS binary64 number contract.
    ///
    /// # Errors
    ///
    /// Returns an input, UTF-8, lexical, budget, duplicate-key, or configured
    /// value-depth error. Input accounting is retained on failure;
    /// staged value usage is committed only after construction succeeds.
    #[cfg(feature = "jcs")]
    pub(in crate::decode) fn decode_jcs_utf8(
        &mut self,
        input: &[u8],
        diagnostic_policy: DiagnosticPolicy,
    ) -> Result<serde_json::Value, JcsDecodeError<R, Q>> {
        let metadata = DecodeMetadata {
            raw_input_bytes: input.len(),
            normalized_input_bytes: None,
            diagnostic_policy,
            number_model: JsonNumberModel::Binary64,
        };
        let has_value_limits = self.session.value_budget().limits().has_limits();
        let attempt = self.session.begin_value();
        Self::decode_jcs_attempt(attempt, input, metadata, true, has_value_limits)
    }

    /// Reads, admits, and decodes one input stream into an owned JCS value.
    ///
    /// # Parameters
    ///
    /// * `input` - Input stream read through EOF, subject to its configured
    ///   budget.
    /// * `diagnostic_policy` - Policy for retaining input-derived error detail.
    ///
    /// # Returns
    ///
    /// The materialized JSON value using the JCS binary64 number contract.
    ///
    /// # Errors
    ///
    /// Returns a read, input-budget, UTF-8, lexical, value-budget,
    /// duplicate-key, or configured value-depth error. Bytes accepted
    /// before a later failure remain charged.
    #[cfg(feature = "jcs")]
    pub(in crate::decode) fn decode_jcs_input<I>(
        &mut self,
        input: &mut I,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Result<serde_json::Value, JcsDecodeError<R, Q>>
    where
        I: Input<Item = u8> + ?Sized,
    {
        let has_value_limits = self.session.value_budget().limits().has_limits();
        let mut attempt = self.session.begin_value();
        let bytes = Self::read_input_bytes(&mut attempt, input, diagnostic_policy)?;
        let raw_input_bytes = bytes.len();
        let metadata = DecodeMetadata {
            raw_input_bytes,
            normalized_input_bytes: None,
            diagnostic_policy,
            number_model: JsonNumberModel::Binary64,
        };
        Self::decode_jcs_attempt(attempt, &bytes, metadata, false, has_value_limits)
    }

    /// Admits a document, builds its JCS value, and commits its transaction.
    ///
    /// # Parameters
    ///
    /// * `attempt` - Attempt owning input and staged value accounting.
    /// * `input` - Complete document bytes admitted before this call.
    /// * `metadata` - Input length and diagnostic policy for structured errors.
    /// * `charge_raw_input` - Whether raw input bytes still need charging.
    /// * `has_value_limits` - Whether the first lexical scan enforces value
    ///   limits.
    ///
    /// # Returns
    ///
    /// The materialized JCS JSON value after admission and accounting commit.
    ///
    /// # Errors
    ///
    /// Returns lexical or budget admission failures, duplicate keys, a
    /// configured value-depth failure, or a commit-time budget failure.
    #[cfg(feature = "jcs")]
    fn decode_jcs_attempt(
        mut attempt: JsonDecodeAttempt<'_, R, Q>,
        input: &[u8],
        metadata: DecodeMetadata,
        charge_raw_input: bool,
        has_value_limits: bool,
    ) -> Result<serde_json::Value, JcsDecodeError<R, Q>> {
        Self::prepare_attempt(&mut attempt, input, metadata, charge_raw_input, has_value_limits)?;
        let mut builder = LexicalJsonValueBuilder::new(JsonNumberModel::Binary64);
        let scan = JsonLexicalScanner::new(attempt.value_transaction_mut(), false)
            .with_number_model(JsonNumberModel::Binary64)
            .scan_with_events(input, |event| {
                let _ = builder.try_accept(event);
            });
        if let Err(error) = scan {
            return Err(JcsDecodeError::Decode(Self::map_lexical_error(error, input, metadata)));
        }
        let value = match builder.finish_guarded() {
            Ok(value) => value,
            Err(JsonValueMaterializationError::DuplicateKey { offset }) => {
                let (line, column) = line_column_at(input, offset);
                return Err(JcsDecodeError::DuplicateKey { line, column });
            }
            Err(JsonValueMaterializationError::InvalidNumber { offset }) => {
                let (line, column) = line_column_at(input, offset);
                let failure = crate::lexical::JsonLexicalFailure {
                    offset,
                    line,
                    column,
                    reason: crate::lexical::JsonLexicalErrorReason::FloatOutOfRange,
                };
                return Err(JcsDecodeError::Decode(Self::map_lexical_error(
                    JsonLexicalError::Syntax(failure),
                    input,
                    metadata,
                )));
            }
        };
        attempt.commit().map_err(|source| {
            JsonDecodeError::budget(
                source,
                JsonDecodeStage::Admission,
                metadata.raw_input_bytes,
                metadata.normalized_input_bytes,
                metadata.diagnostic_policy,
            )
        })?;
        Ok(value.into_value())
    }

    /// Reads and charges bytes for one attempt, allowing one uncharged probe.
    ///
    /// # Parameters
    ///
    /// * `attempt` - Active attempt receiving accepted input-byte charges.
    /// * `input` - Source read until EOF or the first terminal error.
    /// * `diagnostic_policy` - Policy for retaining a read error source.
    ///
    /// # Returns
    ///
    /// The complete byte sequence read from `input`.
    ///
    /// # Errors
    ///
    /// Returns a read or input-budget error with the number of accepted bytes
    /// retained in its metadata. Interrupted reads are retried.
    fn read_input_bytes<I>(
        attempt: &mut JsonDecodeAttempt<'_, R, Q>,
        input: &mut I,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Result<Vec<u8>, JsonDecodeError<R, Q>>
    where
        I: Input<Item = u8> + ?Sized,
    {
        let mut scratch = [0_u8; 8192];
        let mut bytes = Vec::new();
        let mut raw_input_bytes = 0_usize;
        loop {
            let admitted = attempt
                .input_budget()
                .map(|budget| Self::max_admitted_input_bytes(budget.remaining()));
            let read_len = admitted.map_or(scratch.len(), |admitted| {
                if admitted == scratch.len() {
                    scratch.len()
                } else {
                    admitted.saturating_add(1)
                }
            });
            let count = match Input::read(input, &mut scratch[..read_len]) {
                Ok(0) => break,
                Ok(count) => count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    return Err(JsonDecodeError::read(error, raw_input_bytes, diagnostic_policy));
                }
            };
            let charged = admitted.map_or(count, |admitted| count.min(admitted));
            let next_total = raw_input_bytes.checked_add(charged).ok_or_else(|| {
                JsonDecodeError::read(
                    io::Error::new(io::ErrorKind::InvalidData, "input byte count overflow"),
                    raw_input_bytes,
                    diagnostic_policy,
                )
            })?;
            attempt.try_consume_input_bytes(charged).map_err(|source| {
                JsonDecodeError::budget(source, JsonDecodeStage::Input, raw_input_bytes, None, diagnostic_policy)
            })?;
            bytes.extend_from_slice(&scratch[..charged]);
            raw_input_bytes = next_total;
            if count > charged {
                return match attempt.try_consume_input_bytes(1) {
                    Err(source) => Err(JsonDecodeError::budget(
                        source,
                        JsonDecodeStage::Input,
                        raw_input_bytes,
                        None,
                        diagnostic_policy,
                    )),
                    Ok(()) => Err(JsonDecodeError::read(
                        io::Error::new(io::ErrorKind::InvalidData, "input budget probe invariant violated"),
                        raw_input_bytes,
                        diagnostic_policy,
                    )),
                };
            }
        }
        Ok(bytes)
    }

    /// Finds the largest native input length that fits the remaining budget.
    ///
    /// # Parameters
    ///
    /// * `remaining` — The remaining input budget quantity.
    ///
    /// # Returns
    ///
    /// The greatest `usize` input length representable within `remaining` and
    /// the fixed read buffer capacity.
    #[must_use]
    fn max_admitted_input_bytes(remaining: Q) -> usize {
        let mut lower = 0_usize;
        let mut upper = 8192_usize;
        while lower < upper {
            let middle = lower + (upper - lower).div_ceil(2);
            if Q::try_from_usize(middle).is_ok_and(|quantity| quantity <= remaining) {
                lower = middle;
            } else {
                upper = middle - 1;
            }
        }
        lower
    }

    /// Runs admission, an optional root check, Serde materialization, and value
    /// commit for one document.
    ///
    /// # Type Parameters
    ///
    /// * `S` — The Serde seed used to create the result.
    ///
    /// # Parameters
    ///
    /// * `seed` — Deserializer seed for the requested value.
    /// * `input` — Complete JSON document bytes.
    /// * `metadata` — Input and diagnostic metadata.
    /// * `charge_raw_input` — Whether raw input bytes need charging.
    /// * `expected` — Optional required root kind.
    ///
    /// # Returns
    ///
    /// The materialized value after admission and accounting commit.
    ///
    /// # Errors
    ///
    /// Returns an input, admission, root-kind, deserialization, or commit
    /// error.
    fn decode_seed<'de, S>(
        &mut self,
        seed: S,
        input: &'de [u8],
        metadata: DecodeMetadata,
        charge_raw_input: bool,
        expected: Option<JsonRootKind>,
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        let has_value_limits = self.session.value_budget().limits().has_limits();
        let mut attempt = self.session.begin_value();
        let value = Self::decode_seed_attempt(
            &mut attempt,
            seed,
            input,
            metadata,
            charge_raw_input,
            has_value_limits,
            expected,
        )?;
        attempt.commit().map_err(|source| {
            JsonDecodeError::budget(
                source,
                JsonDecodeStage::Admission,
                metadata.raw_input_bytes,
                metadata.normalized_input_bytes,
                metadata.diagnostic_policy,
            )
        })?;
        Ok(value)
    }

    /// Admits one document, checks its optional root kind, and materializes it.
    ///
    /// # Parameters
    ///
    /// * `attempt` - Value transaction that stages input and value usage.
    /// * `seed` - Serde seed used to construct the requested value.
    /// * `input` - Complete JSON document borrowed for `'de`.
    /// * `metadata` - Original input and diagnostic policy information.
    /// * `charge_raw_input` - Whether this attempt must charge raw input bytes.
    /// * `has_value_limits` - Whether value measurements can reject the input.
    /// * `expected` - Optional required top-level object or array kind.
    ///
    /// # Returns
    ///
    /// The value produced by `seed` after the complete document is admitted.
    ///
    /// # Errors
    ///
    /// Returns an error when UTF-8 validation, lexical admission, the optional
    /// root-kind check, or Serde materialization rejects the document. The
    /// error retains the input lengths and diagnostic policy from `metadata`.
    fn decode_seed_attempt<'de, S>(
        attempt: &mut JsonDecodeAttempt<'_, R, Q>,
        seed: S,
        input: &'de [u8],
        metadata: DecodeMetadata,
        charge_raw_input: bool,
        has_value_limits: bool,
        expected: Option<JsonRootKind>,
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        Self::prepare_attempt(attempt, input, metadata, charge_raw_input, has_value_limits)?;
        Self::check_top_level(input, metadata, expected)?;
        let value = deserialize_json_document(seed, input).map_err(|source| {
            JsonDecodeError::deserialize(
                source,
                metadata.raw_input_bytes,
                metadata.normalized_input_bytes,
                metadata.diagnostic_policy,
            )
        })?;
        Ok(value)
    }

    /// Runs admission, an optional root check, and value commit without Serde
    /// materialization.
    ///
    /// # Parameters
    ///
    /// * `input` — Complete JSON document bytes.
    /// * `metadata` — Input and diagnostic metadata.
    /// * `charge_raw_input` — Whether raw input bytes need charging.
    /// * `expected` — Optional required root kind.
    ///
    /// # Returns
    ///
    /// `Ok(())` after admission and accounting commit.
    ///
    /// # Errors
    ///
    /// Returns an input, admission, root-kind, or commit error.
    fn validate(
        &mut self,
        input: &[u8],
        metadata: DecodeMetadata,
        charge_raw_input: bool,
        expected: Option<JsonRootKind>,
    ) -> Result<(), JsonDecodeError<R, Q>> {
        let has_value_limits = self.session.value_budget().limits().has_limits();
        let mut attempt = self.session.begin_value();
        Self::prepare_attempt(&mut attempt, input, metadata, charge_raw_input, has_value_limits)?;
        Self::check_top_level(input, metadata, expected)?;
        attempt.commit().map_err(|source| {
            JsonDecodeError::budget(
                source,
                JsonDecodeStage::Admission,
                metadata.raw_input_bytes,
                metadata.normalized_input_bytes,
                metadata.diagnostic_policy,
            )
        })?;
        Ok(())
    }

    /// Charges input when requested, validates UTF-8, and stages value usage.
    ///
    /// # Parameters
    ///
    /// * `attempt` — Active transaction receiving input and value charges.
    /// * `input` — Complete input document bytes.
    /// * `metadata` — Input and diagnostic metadata.
    /// * `charge_raw_input` — Whether raw input bytes need charging.
    /// * `has_value_limits` — Whether value admission must enforce limits.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the input is charged and admitted.
    ///
    /// # Errors
    ///
    /// Returns an input-budget, UTF-8, lexical, or value-budget error.
    fn prepare_attempt(
        attempt: &mut JsonDecodeAttempt<'_, R, Q>,
        input: &[u8],
        metadata: DecodeMetadata,
        charge_raw_input: bool,
        has_value_limits: bool,
    ) -> Result<(), JsonDecodeError<R, Q>> {
        if charge_raw_input {
            attempt
                .try_consume_input_bytes(metadata.raw_input_bytes)
                .map_err(|source| {
                    JsonDecodeError::budget(
                        source,
                        JsonDecodeStage::Input,
                        metadata.raw_input_bytes,
                        metadata.normalized_input_bytes,
                        metadata.diagnostic_policy,
                    )
                })?;
        }
        std::str::from_utf8(input).map_err(|source| {
            JsonDecodeError::invalid_utf8(source, metadata.raw_input_bytes, metadata.diagnostic_policy)
        })?;
        let admission = if matches!(metadata.number_model, JsonNumberModel::Integer64) {
            admit_json_document(attempt, input, has_value_limits)
        } else {
            admit_json_document_with_number_model(attempt, input, has_value_limits, metadata.number_model)
        };
        admission.map_err(|error| Self::map_lexical_error(error, input, metadata))
    }

    /// Converts a scanner failure while retaining the existing diagnostic
    /// policy.
    ///
    /// # Parameters
    ///
    /// * `error` — Scanner syntax or budget failure.
    /// * `input` — Original input used to recover detailed Serde diagnostics.
    /// * `metadata` — Input lengths and diagnostic policy.
    ///
    /// # Returns
    ///
    /// The corresponding structured decode error.
    fn map_lexical_error(
        error: JsonLexicalError<R, Q>,
        input: &[u8],
        metadata: DecodeMetadata,
    ) -> JsonDecodeError<R, Q> {
        match error {
            JsonLexicalError::Budget(source) => JsonDecodeError::budget(
                source,
                JsonDecodeStage::Admission,
                metadata.raw_input_bytes,
                metadata.normalized_input_bytes,
                metadata.diagnostic_policy,
            ),
            JsonLexicalError::Syntax(source) => {
                let detailed_source = (metadata.diagnostic_policy == DiagnosticPolicy::Detailed).then(|| {
                    from_slice::<&RawValue>(input).err().map_or_else(
                        || Arc::new(source) as Arc<dyn Error + Send + Sync>,
                        |error| Arc::new(error) as Arc<dyn Error + Send + Sync>,
                    )
                });
                JsonDecodeError::invalid_json(
                    source,
                    detailed_source,
                    metadata.raw_input_bytes,
                    metadata.normalized_input_bytes,
                    metadata.diagnostic_policy,
                )
            }
        }
    }

    fn map_validated_lexical_error(
        error: JsonLexicalError<R, Q>,
        raw_input_bytes: usize,
        diagnostic_policy: DiagnosticPolicy,
    ) -> JsonDecodeError<R, Q> {
        match error {
            JsonLexicalError::Budget(source) => JsonDecodeError::budget(
                source,
                JsonDecodeStage::Admission,
                raw_input_bytes,
                None,
                diagnostic_policy,
            ),
            JsonLexicalError::Syntax(source) => {
                let detailed = (diagnostic_policy == DiagnosticPolicy::Detailed)
                    .then(|| Arc::new(source) as Arc<dyn Error + Send + Sync>);
                JsonDecodeError::invalid_json(source, detailed, raw_input_bytes, None, diagnostic_policy)
            }
        }
    }

    /// Enforces an optional top-level object or array contract after admission.
    ///
    /// # Parameters
    ///
    /// * `input` — Admitted UTF-8 JSON document bytes.
    /// * `metadata` — Input lengths and diagnostic policy.
    /// * `expected` — Optional required top-level kind.
    ///
    /// # Returns
    ///
    /// `Ok(())` when no kind is required or the actual kind matches.
    ///
    /// # Errors
    ///
    /// Returns an unexpected-top-level error when the actual kind differs.
    ///
    /// # Panics
    ///
    /// Panics when `input` is not valid UTF-8, which violates the admission
    /// invariant required by this helper.
    fn check_top_level(
        input: &[u8],
        metadata: DecodeMetadata,
        expected: Option<JsonRootKind>,
    ) -> Result<(), JsonDecodeError<R, Q>> {
        let Some(expected) = expected else {
            return Ok(());
        };
        let text = std::str::from_utf8(input).expect("admitted JSON input must be valid UTF-8");
        let actual = JsonRootKind::of_normalized_json(text);
        if actual == expected {
            Ok(())
        } else {
            Err(JsonDecodeError::unexpected_top_level(
                expected,
                actual,
                metadata.raw_input_bytes,
                metadata.normalized_input_bytes,
                metadata.diagnostic_policy,
            ))
        }
    }
}

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
use serde_json::from_slice;
use serde_json::value::RawValue;

use super::DecodeMetadata;
use super::JsonNormalizer;
use super::admit_json_document;
use super::deserialize_json_document;
use super::json_document_decoder::admit_json_document_with_number_model;
use crate::decode::DiagnosticPolicy;
#[cfg(feature = "jcs")]
use crate::decode::JcsDecodeError;
use crate::decode::JsonDecodeError;
use crate::decode::JsonDecodeStage;
use crate::decode::JsonRootKind;
use crate::decode::NormalizedJsonDocument;
use crate::lexical::JsonLexicalError;
#[cfg(feature = "jcs")]
use crate::lexical::JsonLexicalScanner;
use crate::lexical::JsonNumberModel;
#[cfg(feature = "jcs")]
use crate::lexical::line_column_at;
#[cfg(feature = "jcs")]
#[cfg(feature = "jcs")]
use crate::value::internal::JsonValueMaterializationError as JcsMaterializationError;
#[cfg(feature = "jcs")]
use crate::value::internal::LexicalJsonValueBuilder;

/// Shared generic execution core used by both public decoder facades.
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
    #[inline]
    #[must_use]
    pub(in crate::decode) const fn new(session: JsonDecodeSession<'budget, R, Q>) -> Self {
        Self { session }
    }

    /// Returns the cumulative session for read-only inspection.
    #[inline(always)]
    #[must_use]
    pub(in crate::decode) const fn session(&self) -> &JsonDecodeSession<'budget, R, Q> {
        &self.session
    }

    /// Returns mutable access to the cumulative session.
    #[inline(always)]
    #[must_use]
    pub(in crate::decode) const fn session_mut(&mut self) -> &mut JsonDecodeSession<'budget, R, Q> {
        &mut self.session
    }

    /// Consumes the engine and returns its cumulative session.
    #[inline(always)]
    #[must_use]
    pub(in crate::decode) fn into_session(self) -> JsonDecodeSession<'budget, R, Q> {
        self.session
    }

    /// Prepares one string with immediate raw and normalized input accounting.
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
    pub(in crate::decode) fn prepare_utf8<'input>(
        &mut self,
        normalizer: &JsonNormalizer,
        input: &'input [u8],
    ) -> Result<NormalizedJsonDocument<'input>, JsonDecodeError<R, Q>> {
        let raw_input_bytes = input.len();
        let diagnostic_policy = normalizer.policy().diagnostic_policy();
        let mut attempt = self.session.begin_value();
        attempt
            .try_consume_input_bytes(raw_input_bytes)
            .map_err(|source| {
                JsonDecodeError::budget(
                    source,
                    JsonDecodeStage::Input,
                    raw_input_bytes,
                    None,
                    diagnostic_policy,
                )
            })?;
        let input = std::str::from_utf8(input).map_err(|source| {
            JsonDecodeError::invalid_utf8(source, raw_input_bytes, diagnostic_policy)
        })?;
        let normalized = normalizer.normalize_after_raw_charge(input, &mut attempt)?;
        Ok(NormalizedJsonDocument::new(normalized, raw_input_bytes))
    }

    /// Strictly validates and deserializes one complete byte document.
    pub(in crate::decode) fn decode_seed_utf8<'de, S>(
        &mut self,
        seed: S,
        input: &'de [u8],
        diagnostic_policy: DiagnosticPolicy,
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        self.decode_seed_utf8_with_number_model(
            seed,
            input,
            diagnostic_policy,
            JsonNumberModel::Integer64,
        )
    }

    /// Decodes a complete document with an explicit internal number contract.
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
        self.decode_seed(
            seed,
            document.as_str().as_bytes(),
            metadata,
            false,
            expected,
        )
    }

    /// Strictly validates one complete byte document and commits value usage.
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
        let value = Self::decode_seed_attempt(
            &mut attempt,
            seed,
            &bytes,
            metadata,
            false,
            has_value_limits,
            None,
        )?;
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
    /// # Errors
    ///
    /// Returns an input, UTF-8, lexical, budget, duplicate-key, or fixed
    /// materialization-depth error. Input accounting is retained on failure;
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
    /// # Errors
    ///
    /// Returns a read, input-budget, UTF-8, lexical, value-budget,
    /// duplicate-key, or fixed materialization-depth error. Bytes accepted
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
    /// # Errors
    ///
    /// Returns lexical or budget admission failures, duplicate keys, a fixed
    /// materialization-depth failure, or a commit-time budget failure.
    #[cfg(feature = "jcs")]
    fn decode_jcs_attempt(
        mut attempt: JsonDecodeAttempt<'_, R, Q>,
        input: &[u8],
        metadata: DecodeMetadata,
        charge_raw_input: bool,
        has_value_limits: bool,
    ) -> Result<serde_json::Value, JcsDecodeError<R, Q>> {
        Self::prepare_attempt(
            &mut attempt,
            input,
            metadata,
            charge_raw_input,
            has_value_limits,
        )?;
        let mut builder = LexicalJsonValueBuilder::new(JsonNumberModel::Binary64);
        let scan = JsonLexicalScanner::new(attempt.value_transaction_mut(), false)
            .with_number_model(JsonNumberModel::Binary64)
            .scan_with_events(input, |event| builder.accept(event));
        if let Err(error) = scan {
            return Err(JcsDecodeError::Decode(Self::map_lexical_error(
                error, input, metadata,
            )));
        }
        let value = match builder.finish() {
            Ok(value) => value,
            Err(JcsMaterializationError::DuplicateKey { offset }) => {
                let (line, column) = line_column_at(input, offset);
                return Err(JcsDecodeError::DuplicateKey { line, column });
            }
            Err(JcsMaterializationError::DepthLimit { offset, limit }) => {
                let (line, column) = line_column_at(input, offset);
                return Err(JcsDecodeError::MaterializationDepthLimit {
                    limit,
                    line,
                    column,
                });
            }
            Err(JcsMaterializationError::InvalidNumber { offset }) => {
                let (line, column) = line_column_at(input, offset);
                let failure = crate::lexical::JsonLexicalFailure {
                    offset,
                    line,
                    column,
                    reason: crate::lexical::JsonLexicalErrorReason::FloatOutOfRange,
                };
                return Err(JcsDecodeError::Decode(Self::map_lexical_error(
                    JsonLexicalError::Syntax(failure), input, metadata,
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
        Ok(value)
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
                    return Err(JsonDecodeError::read(
                        error,
                        raw_input_bytes,
                        diagnostic_policy,
                    ));
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
                JsonDecodeError::budget(
                    source,
                    JsonDecodeStage::Input,
                    raw_input_bytes,
                    None,
                    diagnostic_policy,
                )
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
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "input budget probe invariant violated",
                        ),
                        raw_input_bytes,
                        diagnostic_policy,
                    )),
                };
            }
        }
        Ok(bytes)
    }

    /// Finds the largest native input length that fits the remaining budget.
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
    fn validate(
        &mut self,
        input: &[u8],
        metadata: DecodeMetadata,
        charge_raw_input: bool,
        expected: Option<JsonRootKind>,
    ) -> Result<(), JsonDecodeError<R, Q>> {
        let has_value_limits = self.session.value_budget().limits().has_limits();
        let mut attempt = self.session.begin_value();
        Self::prepare_attempt(
            &mut attempt,
            input,
            metadata,
            charge_raw_input,
            has_value_limits,
        )?;
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
            JsonDecodeError::invalid_utf8(
                source,
                metadata.raw_input_bytes,
                metadata.diagnostic_policy,
            )
        })?;
        let admission = if matches!(metadata.number_model, JsonNumberModel::Integer64) {
            admit_json_document(attempt, input, has_value_limits)
        } else {
            admit_json_document_with_number_model(
                attempt,
                input,
                has_value_limits,
                metadata.number_model,
            )
        };
        admission.map_err(|error| Self::map_lexical_error(error, input, metadata))
    }

    /// Converts a scanner failure while retaining the existing diagnostic
    /// policy.
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
                let detailed_source = (metadata.diagnostic_policy == DiagnosticPolicy::Detailed)
                    .then(|| {
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

    /// Enforces an optional top-level object or array contract after admission.
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

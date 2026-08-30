// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared resource admission and materialization for JSON decoder facades.

use std::error::Error;
use std::sync::Arc;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonDecodeAttempt;
use qubit_budget::json::JsonDecodeSession;
use serde::de::DeserializeSeed;
use serde_json::from_slice;
use serde_json::value::RawValue;

use super::DecodeMetadata;
use super::JsonNormalizer;
use super::admit_json_document;
use super::deserialize_json_document;
use crate::decode::DiagnosticPolicy;
use crate::decode::JsonDecodeError;
use crate::decode::JsonDecodeStage;
use crate::decode::JsonRootKind;
use crate::decode::NormalizedJsonDocument;
use crate::lexical::JsonLexicalError;

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
        attempt.try_consume_input_bytes(raw_input_bytes).map_err(|source| {
            JsonDecodeError::budget(source, JsonDecodeStage::Input, raw_input_bytes, None, diagnostic_policy)
        })?;
        let input = std::str::from_utf8(input)
            .map_err(|source| JsonDecodeError::invalid_utf8(source, raw_input_bytes, diagnostic_policy))?;
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
        self.decode_seed_utf8_with_top_level(seed, input, diagnostic_policy, None)
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
        };
        self.decode_seed(seed, document.as_str().as_bytes(), metadata, false, expected)
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
        };
        self.validate(document.as_str().as_bytes(), metadata, false, expected)
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
        Self::prepare_attempt(&mut attempt, input, metadata, charge_raw_input, has_value_limits)?;
        Self::check_top_level(input, metadata, expected)?;
        let value = deserialize_json_document(seed, input).map_err(|source| {
            JsonDecodeError::deserialize(
                source,
                metadata.raw_input_bytes,
                metadata.normalized_input_bytes,
                metadata.diagnostic_policy,
            )
        })?;
        attempt.commit();
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
        Self::prepare_attempt(&mut attempt, input, metadata, charge_raw_input, has_value_limits)?;
        Self::check_top_level(input, metadata, expected)?;
        attempt.commit();
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
            JsonDecodeError::invalid_utf8(source, metadata.raw_input_bytes, metadata.diagnostic_policy)
        })?;
        admit_json_document(attempt, input, has_value_limits).map_err(|error| match error {
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
        })
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

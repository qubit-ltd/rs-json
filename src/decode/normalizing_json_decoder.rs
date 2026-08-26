// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the explicit normalization facade for JSON decoding.

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde::de::DeserializeSeed;
use serde_json::Value;

use super::JsonDecodeError;
use super::JsonRootKind;
use super::NormalizedJsonDocument;
use super::NormalizingJsonDecodePolicy;
use super::internal::JsonDecodeEngine;
use super::internal::JsonNormalizer;
use super::internal::TypedSeed;

/// Normalizes non-fully-trusted text before decoding complete JSON documents.
///
/// Owned convenience methods prepare and decode in one call. Callers needing
/// borrowed results, custom seeds, or repeated materialization first create a
/// [`NormalizedJsonDocument`] and then use a document decoding method.
///
/// # Examples
///
/// ```
/// use qubit_budget::json::JsonDecodeLimits;
/// use qubit_json::decode::NormalizingJsonDecodePolicy;
/// use qubit_json::decode::NormalizingJsonDecoder;
/// use serde_json::Value;
///
/// let mut decoder = NormalizingJsonDecoder::owned(
///     NormalizingJsonDecodePolicy::builder().build(),
///     JsonDecodeLimits::new(),
/// );
/// let value = decoder.decode_str::<Value>("```json\n{\"ok\":true}\n```")?;
/// assert_eq!(value["ok"], true);
/// # Ok::<(), qubit_json::decode::JsonDecodeError>(())
/// ```
#[derive(Debug)]
pub struct NormalizingJsonDecoder<'budget, R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    /// Configured normalization pipeline.
    normalizer: JsonNormalizer,
    /// Shared generic decoding and accounting core.
    engine: JsonDecodeEngine<'budget, R, Q>,
}

impl<R, Q> NormalizingJsonDecoder<'static, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a decoder with an owned session built from explicit limits.
    #[inline]
    #[must_use]
    pub fn owned_with_limits(policy: NormalizingJsonDecodePolicy, limits: JsonDecodeLimits<R, Q>) -> Self {
        Self::new(policy, JsonDecodeSession::owned(limits))
    }
}

impl NormalizingJsonDecoder<'static, JsonResource, usize> {
    /// Creates a standard decoder with an owned standard JSON session.
    #[inline]
    #[must_use]
    pub fn owned(policy: NormalizingJsonDecodePolicy, limits: JsonDecodeLimits) -> Self {
        Self::new(policy, JsonDecodeSession::owned(limits))
    }
}

impl<'budget, R, Q> NormalizingJsonDecoder<'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a decoder around a reusable caller-provided session.
    #[inline]
    #[must_use]
    pub const fn new(policy: NormalizingJsonDecodePolicy, session: JsonDecodeSession<'budget, R, Q>) -> Self {
        Self {
            normalizer: JsonNormalizer::new(policy),
            engine: JsonDecodeEngine::new(session),
        }
    }

    /// Returns the cumulative session for read-only inspection.
    #[inline(always)]
    #[must_use]
    pub const fn session(&self) -> &JsonDecodeSession<'budget, R, Q> {
        self.engine.session()
    }

    /// Returns mutable access to the cumulative session.
    #[inline(always)]
    #[must_use]
    pub const fn session_mut(&mut self) -> &mut JsonDecodeSession<'budget, R, Q> {
        self.engine.session_mut()
    }

    /// Consumes the decoder and returns its cumulative session.
    #[inline(always)]
    #[must_use]
    pub fn into_session(self) -> JsonDecodeSession<'budget, R, Q> {
        self.engine.into_session()
    }

    /// Returns the immutable normalization and diagnostic policy.
    #[inline(always)]
    #[must_use]
    pub const fn policy(&self) -> &NormalizingJsonDecodePolicy {
        self.normalizer.policy()
    }

    /// Normalizes one string and immediately charges its input budgets.
    ///
    /// The returned document may borrow `input`. Later document decoding does
    /// not charge its input again and commits only decoded-value usage.
    pub fn prepare_str<'input>(
        &mut self,
        input: &'input str,
    ) -> Result<NormalizedJsonDocument<'input>, JsonDecodeError<R, Q>> {
        self.engine.prepare_str(&self.normalizer, input)
    }

    /// Charges raw bytes, validates UTF-8, and normalizes one byte slice.
    ///
    /// Raw input usage remains charged when UTF-8 validation or normalization
    /// fails. The returned document may borrow the original byte slice.
    pub fn prepare_utf8<'input>(
        &mut self,
        input: &'input [u8],
    ) -> Result<NormalizedJsonDocument<'input>, JsonDecodeError<R, Q>> {
        self.engine.prepare_utf8(&self.normalizer, input)
    }

    /// Decodes one prepared document and permits results borrowing it.
    pub fn decode_document<'de, T>(
        &mut self,
        document: &'de NormalizedJsonDocument<'_>,
    ) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.decode_document_seed(document, TypedSeed::new())
    }

    /// Decodes one prepared document through a caller-provided Serde seed.
    pub fn decode_document_seed<'de, S>(
        &mut self,
        document: &'de NormalizedJsonDocument<'_>,
        seed: S,
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        self.engine
            .decode_document_seed(document, seed, self.policy().diagnostic_policy(), None)
    }

    /// Decodes one prepared document while requiring a top-level object.
    pub fn decode_object_document<'de, T>(
        &mut self,
        document: &'de NormalizedJsonDocument<'_>,
    ) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.engine.decode_document_seed(
            document,
            TypedSeed::new(),
            self.policy().diagnostic_policy(),
            Some(JsonRootKind::Object),
        )
    }

    /// Decodes one prepared document while requiring a top-level array.
    pub fn decode_array_document<'de, T>(
        &mut self,
        document: &'de NormalizedJsonDocument<'_>,
    ) -> Result<Vec<T>, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.engine.decode_document_seed(
            document,
            TypedSeed::new(),
            self.policy().diagnostic_policy(),
            Some(JsonRootKind::Array),
        )
    }

    /// Validates a prepared document and commits its decoded-value usage.
    pub fn validate_document(&mut self, document: &NormalizedJsonDocument<'_>) -> Result<(), JsonDecodeError<R, Q>> {
        self.engine
            .validate_document(document, self.policy().diagnostic_policy(), None)
    }

    /// Normalizes and decodes one string into an owned target value.
    pub fn decode_str<T>(&mut self, input: &str) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
    {
        let document = self.prepare_str(input)?;
        self.decode_document(&document)
    }

    /// Normalizes and decodes one UTF-8 byte slice into an owned target value.
    pub fn decode_utf8<T>(&mut self, input: &[u8]) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
    {
        let document = self.prepare_utf8(input)?;
        self.decode_document(&document)
    }

    /// Normalizes and decodes one string while requiring a top-level object.
    pub fn decode_object<T>(&mut self, input: &str) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
    {
        let document = self.prepare_str(input)?;
        self.decode_object_document(&document)
    }

    /// Normalizes and decodes one string while requiring a top-level array.
    pub fn decode_array<T>(&mut self, input: &str) -> Result<Vec<T>, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
    {
        let document = self.prepare_str(input)?;
        self.decode_array_document(&document)
    }

    /// Normalizes and decodes one string into a dynamic JSON value.
    pub fn decode_value(&mut self, input: &str) -> Result<Value, JsonDecodeError<R, Q>> {
        self.decode_str(input)
    }
}

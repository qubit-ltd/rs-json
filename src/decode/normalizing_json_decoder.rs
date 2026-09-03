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
/// let mut decoder = NormalizingJsonDecoder::with_limits(
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
    /// Creates a normalizing decoder with a cumulative session built from
    /// explicit limits.
    ///
    /// # Parameters
    ///
    /// * `policy` - Normalization and diagnostic behavior applied before
    ///   decoding.
    /// * `limits` - Input and decoded-value limits used by the cumulative
    ///   session.
    ///
    /// # Returns
    ///
    /// A decoder whose accounting starts empty and is constrained by `limits`.
    #[inline]
    #[must_use]
    pub fn with_limits(policy: NormalizingJsonDecodePolicy, limits: JsonDecodeLimits<R, Q>) -> Self {
        Self::new(policy, JsonDecodeSession::from_limits(limits))
    }
}

impl<'budget, R, Q> NormalizingJsonDecoder<'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a decoder around a reusable caller-provided session.
    ///
    /// # Parameters
    ///
    /// * `policy` - Normalization and diagnostic behavior applied before
    ///   decoding.
    /// * `session` - Cumulative session receiving input and decoded-value
    ///   charges.
    ///
    /// # Returns
    ///
    /// A decoder that owns `session` until it is consumed by
    /// [`Self::into_session`].
    #[inline]
    #[must_use]
    pub const fn new(policy: NormalizingJsonDecodePolicy, session: JsonDecodeSession<'budget, R, Q>) -> Self {
        Self {
            normalizer: JsonNormalizer::new(policy),
            engine: JsonDecodeEngine::new(session),
        }
    }

    /// Returns the cumulative session for read-only inspection.
    ///
    /// The returned reference exposes charges accumulated by completed
    /// preparation and decoding operations.
    ///
    /// # Returns
    ///
    /// A shared reference to the cumulative session.
    #[inline(always)]
    #[must_use]
    pub const fn session(&self) -> &JsonDecodeSession<'budget, R, Q> {
        self.engine.session()
    }

    /// Returns mutable access to the cumulative session.
    ///
    /// Mutating the session changes the limits and accounting state used by
    /// subsequent operations.
    ///
    /// # Returns
    ///
    /// A mutable reference to the cumulative session.
    #[inline(always)]
    #[must_use]
    pub const fn session_mut(&mut self) -> &mut JsonDecodeSession<'budget, R, Q> {
        self.engine.session_mut()
    }

    /// Consumes the decoder and returns its cumulative session.
    ///
    /// Ownership of all accumulated accounting state is transferred without
    /// resetting it or performing another decode.
    ///
    /// # Returns
    ///
    /// The session previously owned by this decoder.
    #[inline(always)]
    #[must_use]
    pub fn into_session(self) -> JsonDecodeSession<'budget, R, Q> {
        self.engine.into_session()
    }

    /// Returns the immutable normalization and diagnostic policy.
    ///
    /// The returned reference remains tied to this decoder and controls how
    /// future input is normalized and how input-derived failures are retained.
    ///
    /// # Returns
    ///
    /// A shared reference to the configured policy.
    #[inline(always)]
    #[must_use]
    pub const fn policy(&self) -> &NormalizingJsonDecodePolicy {
        self.normalizer.policy()
    }

    /// Normalizes one string and immediately charges its input budgets.
    ///
    /// The returned document may borrow `input`. Later document decoding does
    /// not charge its input again and commits only decoded-value usage.
    ///
    /// # Parameters
    ///
    /// * `input` - JSON text to normalize and charge.
    ///
    /// # Returns
    ///
    /// A normalized document that may borrow `input`.
    ///
    /// # Errors
    ///
    /// Returns a structured error when input accounting, UTF-8 validation, or
    /// normalization fails.
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
    ///
    /// # Parameters
    ///
    /// * `input` - UTF-8 byte slice to validate, normalize, and charge.
    ///
    /// # Returns
    ///
    /// A normalized document that may borrow `input`.
    ///
    /// # Errors
    ///
    /// Returns a structured error when input accounting, UTF-8 validation, or
    /// normalization fails.
    pub fn prepare_utf8<'input>(
        &mut self,
        input: &'input [u8],
    ) -> Result<NormalizedJsonDocument<'input>, JsonDecodeError<R, Q>> {
        self.engine.prepare_utf8(&self.normalizer, input)
    }

    /// Decodes one precharged document and permits results borrowing it.
    ///
    /// Preparing the document has already committed its raw and normalized
    /// input usage. This method commits only the decoded-value usage.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Target type deserialized from the prepared document.
    ///
    /// # Parameters
    ///
    /// * `document` - Precharged normalized document that outlives the returned
    ///   value.
    ///
    /// # Returns
    ///
    /// The deserialized value on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error when decoded-value accounting, parsing, or
    /// deserialization fails.
    pub fn decode_precharged_document<'de, T>(
        &mut self,
        document: &'de NormalizedJsonDocument<'_>,
    ) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.decode_precharged_document_seed(document, TypedSeed::new())
    }

    /// Decodes one precharged document through a caller-provided Serde seed.
    ///
    /// Preparing the document has already committed its raw and normalized
    /// input usage. This method commits only the decoded-value usage.
    ///
    /// # Type Parameters
    ///
    /// * `S` - Seed controlling construction of the decoded value.
    ///
    /// # Parameters
    ///
    /// * `document` - Precharged normalized document.
    /// * `seed` - Serde seed used to deserialize the document.
    ///
    /// # Returns
    ///
    /// The value produced by `seed`.
    ///
    /// # Errors
    ///
    /// Returns a structured error when decoded-value accounting, parsing, or
    /// seeded deserialization fails.
    pub fn decode_precharged_document_seed<'de, S>(
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

    /// Decodes one precharged document while requiring a top-level object.
    ///
    /// Preparing the document has already committed its raw and normalized
    /// input usage. This method commits only the decoded-value usage.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Target type deserialized from the object document.
    ///
    /// # Parameters
    ///
    /// * `document` - Precharged document that outlives the returned value.
    ///
    /// # Returns
    ///
    /// The deserialized object value on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error for accounting, parsing, top-level-kind, or
    /// deserialization failures.
    pub fn decode_precharged_object_document<'de, T>(
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

    /// Decodes one precharged document while requiring a top-level array.
    ///
    /// Preparing the document has already committed its raw and normalized
    /// input usage. This method commits only the decoded-value usage.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Element type deserialized from the array.
    ///
    /// # Parameters
    ///
    /// * `document` - Precharged document that outlives the returned elements.
    ///
    /// # Returns
    ///
    /// The decoded array elements on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error for accounting, parsing, top-level-kind, or
    /// deserialization failures.
    pub fn decode_precharged_array_document<'de, T>(
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

    /// Validates a precharged document and commits its decoded-value usage.
    ///
    /// Preparing the document has already committed its raw and normalized
    /// input usage. This method commits only the decoded-value usage.
    ///
    /// # Parameters
    ///
    /// * `document` - Precharged document whose JSON syntax is validated.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the normalized document is valid and its decoded-value
    /// usage is committed.
    ///
    /// # Errors
    ///
    /// Returns a structured error when decoded-value accounting or JSON
    /// validation fails.
    pub fn validate_precharged_document(
        &mut self,
        document: &NormalizedJsonDocument<'_>,
    ) -> Result<(), JsonDecodeError<R, Q>> {
        self.engine
            .validate_document(document, self.policy().diagnostic_policy(), None)
    }

    /// Normalizes and decodes one string into an owned target value.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Owned target type deserialized from the normalized document.
    ///
    /// # Parameters
    ///
    /// * `input` - JSON text to normalize and decode.
    ///
    /// # Returns
    ///
    /// The owned deserialized value on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error when normalization, accounting, parsing, or
    /// deserialization fails.
    pub fn decode_str<T>(&mut self, input: &str) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
    {
        let document = self.prepare_str(input)?;
        self.decode_precharged_document(&document)
    }

    /// Normalizes and decodes one UTF-8 byte slice into an owned target value.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Owned target type deserialized from the normalized document.
    ///
    /// # Parameters
    ///
    /// * `input` - UTF-8 JSON bytes to normalize and decode.
    ///
    /// # Returns
    ///
    /// The owned deserialized value on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error when normalization, accounting, UTF-8
    /// validation, parsing, or deserialization fails.
    pub fn decode_utf8<T>(&mut self, input: &[u8]) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
    {
        let document = self.prepare_utf8(input)?;
        self.decode_precharged_document(&document)
    }

    /// Normalizes and decodes one string while requiring a top-level object.
    ///
    /// # Parameters
    ///
    /// * `input` - JSON text whose normalized root must be an object.
    ///
    /// # Returns
    ///
    /// The owned deserialized object value on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error for normalization, accounting, parsing,
    /// top-level-kind, or deserialization failures.
    pub fn decode_object_str<T>(&mut self, input: &str) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
    {
        let document = self.prepare_str(input)?;
        self.decode_precharged_object_document(&document)
    }

    /// Normalizes and decodes one UTF-8 byte slice while requiring a top-level
    /// object.
    ///
    /// # Parameters
    ///
    /// * `input` - UTF-8 JSON bytes whose normalized root must be an object.
    ///
    /// # Returns
    ///
    /// The owned deserialized object value on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error for normalization, accounting, UTF-8
    /// validation, parsing, top-level-kind, or deserialization failures.
    pub fn decode_object_utf8<T>(&mut self, input: &[u8]) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
    {
        let document = self.prepare_utf8(input)?;
        self.decode_precharged_object_document(&document)
    }

    /// Normalizes and decodes one string while requiring a top-level array.
    ///
    /// # Parameters
    ///
    /// * `input` - JSON text whose normalized root must be an array.
    ///
    /// # Returns
    ///
    /// The owned decoded array elements on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error for normalization, accounting, parsing,
    /// top-level-kind, or deserialization failures.
    pub fn decode_array_str<T>(&mut self, input: &str) -> Result<Vec<T>, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
    {
        let document = self.prepare_str(input)?;
        self.decode_precharged_array_document(&document)
    }

    /// Normalizes and decodes one UTF-8 byte slice while requiring a top-level
    /// array.
    ///
    /// # Parameters
    ///
    /// * `input` - UTF-8 JSON bytes whose normalized root must be an array.
    ///
    /// # Returns
    ///
    /// The owned decoded array elements on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error for normalization, accounting, UTF-8
    /// validation, parsing, top-level-kind, or deserialization failures.
    pub fn decode_array_utf8<T>(&mut self, input: &[u8]) -> Result<Vec<T>, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
    {
        let document = self.prepare_utf8(input)?;
        self.decode_precharged_array_document(&document)
    }

    /// Normalizes and decodes one string into a dynamic JSON value.
    ///
    /// # Parameters
    ///
    /// * `input` - JSON text to normalize and materialize as a value tree.
    ///
    /// # Returns
    ///
    /// The materialized JSON value on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error when normalization, accounting, parsing, or
    /// value construction fails.
    pub fn decode_value(&mut self, input: &str) -> Result<Value, JsonDecodeError<R, Q>> {
        self.decode_str(input)
    }
}

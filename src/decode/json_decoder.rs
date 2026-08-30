// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Stateful strict JSON decoding with caller-owned resource accounting.

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use serde::Deserialize;
use serde::de::DeserializeSeed;

use super::DiagnosticPolicy;
use super::JsonDecodeError;
use super::JsonRootKind;
use super::internal::JsonDecodeEngine;
use super::internal::TypedSeed;

/// Strictly decodes complete JSON documents while retaining cumulative usage.
///
/// This facade performs no normalization. It accepts integers from `i64::MIN`
/// through `u64::MAX`, requires finite floating-point values, supports values
/// borrowing from its input, and exposes caller-provided Serde seeds.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::JsonDecoder;
/// use serde_json::Value;
///
/// let mut decoder = JsonDecoder::unlimited();
/// let value = decoder.decode_str::<Value>(r#"{"ok":true}"#)?;
/// assert_eq!(value["ok"], true);
/// # Ok::<(), qubit_json::decode::JsonDecodeError>(())
/// ```
#[derive(Debug)]
pub struct JsonDecoder<'budget, R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    /// Diagnostic detail retained for input-derived failures.
    diagnostic_policy: DiagnosticPolicy,
    /// Shared generic decoding and accounting core.
    engine: JsonDecodeEngine<'budget, R, Q>,
}

impl<R, Q> JsonDecoder<'static, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a decoder with a cumulative session built from explicit limits.
    ///
    /// # Parameters
    ///
    /// * `limits` - Input and decoded-value limits used by the cumulative
    ///   session.
    ///
    /// # Returns
    ///
    /// A decoder whose accounting starts empty and is constrained by `limits`.
    #[inline(always)]
    #[must_use]
    pub fn with_limits(limits: JsonDecodeLimits<R, Q>) -> Self {
        Self::new(JsonDecodeSession::from_limits(limits))
    }
}

impl JsonDecoder<'static, JsonResource, usize> {
    /// Creates a decoder with no configured input or value limits.
    ///
    /// # Returns
    ///
    /// A decoder using the standard resource identities with all limits
    /// disabled.
    #[inline(always)]
    #[must_use]
    pub fn unlimited() -> Self {
        Self::with_limits(JsonDecodeLimits::new())
    }
}

impl<'budget, R, Q> JsonDecoder<'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a strict decoder around a reusable cumulative session.
    ///
    /// # Parameters
    ///
    /// * `session` - Cumulative session that receives input and decoded-value
    ///   charges.
    ///
    /// # Returns
    ///
    /// A decoder that owns `session` until it is consumed by
    /// [`Self::into_session`].
    #[inline]
    #[must_use]
    pub const fn new(session: JsonDecodeSession<'budget, R, Q>) -> Self {
        Self {
            diagnostic_policy: DiagnosticPolicy::Redacted,
            engine: JsonDecodeEngine::new(session),
        }
    }

    /// Configures whether input-derived error sources are retained.
    ///
    /// The default is [`DiagnosticPolicy::Redacted`]. Selecting
    /// [`DiagnosticPolicy::Detailed`] may retain source errors containing
    /// fragments or structural details derived from the input.
    ///
    /// # Parameters
    ///
    /// * `policy` - Diagnostic retention policy for failures produced by this
    ///   decoder.
    ///
    /// # Returns
    ///
    /// The decoder with the requested policy; its existing session is retained.
    #[inline(always)]
    #[must_use]
    pub const fn with_diagnostic_policy(mut self, policy: DiagnosticPolicy) -> Self {
        self.diagnostic_policy = policy;
        self
    }

    /// Returns the configured diagnostic policy without changing the decoder.
    ///
    /// # Returns
    ///
    /// The policy used when constructing input-derived decode errors.
    #[inline(always)]
    #[must_use]
    pub const fn diagnostic_policy(&self) -> DiagnosticPolicy {
        self.diagnostic_policy
    }

    /// Returns the cumulative session for read-only inspection.
    ///
    /// The returned reference is borrowed from the decoder and exposes the
    /// charges accumulated by completed operations.
    ///
    /// # Returns
    ///
    /// A shared reference to the decoder's cumulative session.
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
    /// A mutable reference tied to the decoder's lifetime.
    #[inline(always)]
    #[must_use]
    pub const fn session_mut(&mut self) -> &mut JsonDecodeSession<'budget, R, Q> {
        self.engine.session_mut()
    }

    /// Consumes the decoder and returns its cumulative session.
    ///
    /// This transfers ownership of all accumulated accounting state without
    /// performing another decode or resetting the session.
    ///
    /// # Returns
    ///
    /// The session previously owned by this decoder.
    #[inline(always)]
    #[must_use]
    pub fn into_session(self) -> JsonDecodeSession<'budget, R, Q> {
        self.engine.into_session()
    }

    /// Decodes one complete JSON string and permits results borrowing `input`.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Target type deserialized from the complete document.
    ///
    /// # Parameters
    ///
    /// * `input` - UTF-8 JSON text. The returned value may borrow from it.
    ///
    /// # Returns
    ///
    /// The deserialized value on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error when input accounting, UTF-8 validation,
    /// JSON parsing, or Serde deserialization fails.
    pub fn decode_str<'de, T>(&mut self, input: &'de str) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.decode_seed_str(TypedSeed::new(), input)
    }

    /// Decodes one complete UTF-8 JSON byte slice and permits borrowed results.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Target type deserialized from the complete document.
    ///
    /// # Parameters
    ///
    /// * `input` - Complete UTF-8 JSON bytes. The returned value may borrow
    ///   from this slice.
    ///
    /// # Returns
    ///
    /// The deserialized value on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error when accounting, UTF-8 validation, JSON
    /// parsing, or Serde deserialization fails.
    pub fn decode_utf8<'de, T>(&mut self, input: &'de [u8]) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.decode_seed_utf8(TypedSeed::new(), input)
    }

    /// Decodes a JSON string through a caller-provided Serde seed.
    ///
    /// # Type Parameters
    ///
    /// * `S` - Seed controlling construction of the decoded value.
    ///
    /// # Parameters
    ///
    /// * `seed` - Serde seed used to deserialize the document.
    /// * `input` - Complete JSON text, which the seed may borrow from.
    ///
    /// # Returns
    ///
    /// The value produced by `seed`.
    ///
    /// # Errors
    ///
    /// Returns a structured error when accounting, parsing, or seeded
    /// deserialization fails.
    pub fn decode_seed_str<'de, S>(&mut self, seed: S, input: &'de str) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        self.decode_seed_utf8(seed, input.as_bytes())
    }

    /// Decodes a UTF-8 byte slice through a caller-provided Serde seed.
    ///
    /// # Type Parameters
    ///
    /// * `S` - Seed controlling construction of the decoded value.
    ///
    /// # Parameters
    ///
    /// * `seed` - Serde seed used to deserialize the document.
    /// * `input` - Complete UTF-8 JSON bytes, which the seed may borrow from.
    ///
    /// # Returns
    ///
    /// The value produced by `seed`.
    ///
    /// # Errors
    ///
    /// Returns a structured error when accounting, UTF-8 validation, parsing,
    /// or seeded deserialization fails.
    pub fn decode_seed_utf8<'de, S>(&mut self, seed: S, input: &'de [u8]) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        self.engine.decode_seed_utf8(seed, input, self.diagnostic_policy)
    }

    /// Decodes a complete JSON string while requiring a top-level object.
    ///
    /// The top-level check is performed before the decoded value is committed,
    /// so an array, scalar, or otherwise valid non-object document is rejected.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Target type deserialized from the object document.
    ///
    /// # Parameters
    ///
    /// * `input` - Complete JSON text, which the returned value may borrow.
    ///
    /// # Returns
    ///
    /// The deserialized object value on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error for accounting, parsing, top-level-kind, or
    /// deserialization failures.
    pub fn decode_object_str<'de, T>(&mut self, input: &'de str) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.decode_object_utf8(input.as_bytes())
    }

    /// Decodes a complete UTF-8 byte slice while requiring a top-level object.
    ///
    /// A syntactically valid array or scalar is rejected by the top-level
    /// constraint before the decoded value is committed.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Target type deserialized from the object document.
    ///
    /// # Parameters
    ///
    /// * `input` - Complete UTF-8 JSON bytes, which the returned value may
    ///   borrow.
    ///
    /// # Returns
    ///
    /// The deserialized object value on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error for accounting, UTF-8 validation, parsing,
    /// top-level-kind, or deserialization failures.
    pub fn decode_object_utf8<'de, T>(&mut self, input: &'de [u8]) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.engine.decode_seed_utf8_with_top_level(
            TypedSeed::new(),
            input,
            self.diagnostic_policy,
            Some(JsonRootKind::Object),
        )
    }

    /// Decodes a complete JSON string while requiring a top-level array.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Element type deserialized from the array.
    ///
    /// # Parameters
    ///
    /// * `input` - Complete JSON text, which the returned elements may borrow.
    ///
    /// # Returns
    ///
    /// The decoded array elements on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error for accounting, parsing, top-level-kind, or
    /// deserialization failures.
    pub fn decode_array_str<'de, T>(&mut self, input: &'de str) -> Result<Vec<T>, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.decode_array_utf8(input.as_bytes())
    }

    /// Decodes a complete UTF-8 byte slice while requiring a top-level array.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Element type deserialized from the array.
    ///
    /// # Parameters
    ///
    /// * `input` - Complete UTF-8 JSON bytes, which the returned elements may
    ///   borrow.
    ///
    /// # Returns
    ///
    /// The decoded array elements on success.
    ///
    /// # Errors
    ///
    /// Returns a structured error for accounting, UTF-8 validation, parsing,
    /// top-level-kind, or deserialization failures.
    pub fn decode_array_utf8<'de, T>(&mut self, input: &'de [u8]) -> Result<Vec<T>, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.engine.decode_seed_utf8_with_top_level(
            TypedSeed::new(),
            input,
            self.diagnostic_policy,
            Some(JsonRootKind::Array),
        )
    }

    /// Validates and accounts for one complete JSON string without
    /// materializing a target value.
    ///
    /// # Parameters
    ///
    /// * `input` - Complete JSON text to validate and account for.
    ///
    /// # Errors
    ///
    /// Returns a structured error when accounting, parsing, or UTF-8
    /// validation fails. No target value is allocated.
    pub fn validate_str(&mut self, input: &str) -> Result<(), JsonDecodeError<R, Q>> {
        self.validate_utf8(input.as_bytes())
    }

    /// Validates and accounts for one complete UTF-8 JSON byte slice without
    /// materializing a target value.
    ///
    /// # Parameters
    ///
    /// * `input` - Complete UTF-8 JSON bytes to validate and account for.
    ///
    /// # Errors
    ///
    /// Returns a structured error when accounting, UTF-8 validation, or JSON
    /// parsing fails. No target value is allocated.
    pub fn validate_utf8(&mut self, input: &[u8]) -> Result<(), JsonDecodeError<R, Q>> {
        self.engine.validate_utf8(input, self.diagnostic_policy)
    }
}

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
use qubit_io::ByteInput;
use qubit_io::Input;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde::de::DeserializeSeed;

use super::DiagnosticPolicy;
use super::JsonDecodeError;
use super::JsonDecodeNumberModel;
use super::JsonNumberPolicy;
use super::JsonNumberToken;
use super::JsonPathSegment;
use super::JsonRootKind;
use super::JsonValidatedDecodeError;
use super::JsonValidatedDecodeOptions;
use super::internal::JsonDecodeEngine;
use super::internal::TypedSeed;
use crate::value::SafeJsonValue;

/// Strictly decodes complete JSON documents while retaining cumulative usage.
///
/// This facade performs no normalization. It accepts integers from `i64::MIN`
/// through `u64::MAX`, requires finite floating-point values, supports values
/// borrowing from its input, and exposes caller-provided Serde seeds.
///
/// # Type Parameters
///
/// * `'budget` - Lifetime of the resource budget borrowed by the decoder's
///   cumulative session.
/// * `R` - Resource identity type tracked by the session.
/// * `Q` - Quantity type used for resource limits and accounting.
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
    /// Decodes one complete document into a guarded value and rejects duplicate
    /// keys.
    ///
    /// # Parameters
    ///
    /// * `input` - Complete UTF-8 JSON document.
    ///
    /// # Returns
    ///
    /// The protected owned value after input and value accounting succeed.
    ///
    /// # Errors
    ///
    /// Returns a structured error for invalid UTF-8, syntax, numeric range,
    /// duplicate keys, or configured resource limits.
    pub fn decode_value_utf8(&mut self, input: &[u8]) -> Result<SafeJsonValue, JsonDecodeError<R, Q>> {
        self.engine.decode_value_utf8(input, self.diagnostic_policy)
    }

    /// UTF-8 string form of [`Self::decode_value_utf8`].
    ///
    /// # Parameters
    ///
    /// * `input` - Complete JSON document text.
    ///
    /// # Returns
    ///
    /// The protected owned value.
    ///
    /// # Errors
    ///
    /// Returns a structured error for syntax, numeric range, duplicate keys,
    /// or configured resource limits.
    pub fn decode_value_str(&mut self, input: &str) -> Result<SafeJsonValue, JsonDecodeError<R, Q>> {
        self.decode_value_utf8(input.as_bytes())
    }

    /// Reads one unique-key document through EOF and decodes it into a guarded
    /// value.
    ///
    /// Reads in bounded chunks, with at most one probe byte beyond an active
    /// input limit. Accepted bytes remain charged when later validation fails.
    ///
    /// # Parameters
    ///
    /// * `input` - Byte input read through EOF.
    ///
    /// # Returns
    ///
    /// The protected owned value.
    ///
    /// # Errors
    ///
    /// Returns read, input-budget, UTF-8, syntax, numeric-range, duplicate-key,
    /// or value-budget failures.
    pub fn decode_value_input<I: ByteInput + ?Sized>(
        &mut self,
        input: &mut I,
    ) -> Result<SafeJsonValue, JsonDecodeError<R, Q>> {
        self.engine.decode_value_input(input, self.diagnostic_policy)
    }

    /// Decodes one document while applying a caller policy to each source
    /// number.
    ///
    /// The callback runs in source order after a number token is syntactically
    /// complete and before its configured representation range is checked. It
    /// receives the typed path and borrowed source spelling; its first error
    /// stops scanning immediately. Object keys must be unique after decoding.
    ///
    /// # Parameters
    ///
    /// * `input` - Complete UTF-8 JSON document.
    /// * `options` - Root, number model, and per-document source-token limits.
    /// * `number_policy` - Synchronous policy called for each complete number.
    ///
    /// # Returns
    ///
    /// The protected owned value after complete scanning and accounting.
    ///
    /// # Errors
    ///
    /// Returns structured decode errors, callback errors with source offsets,
    /// exact-safe-integer errors, or validated-limit failures.
    pub fn decode_unique_validated_utf8<E, F>(
        &mut self,
        input: &[u8],
        options: &JsonValidatedDecodeOptions,
        number_policy: F,
    ) -> Result<SafeJsonValue, JsonValidatedDecodeError<E, R, Q>>
    where
        F: for<'call> FnMut(&'call [JsonPathSegment], JsonNumberToken<'call>) -> Result<JsonNumberPolicy, E>,
    {
        self.engine
            .decode_unique_validated_utf8(input, options, number_policy, self.diagnostic_policy)
    }

    /// UTF-8 string form of [`Self::decode_unique_validated_utf8`].
    ///
    /// # Parameters
    ///
    /// * `input` - Complete JSON document text.
    /// * `options` - Root, number model, and per-document source-token limits.
    /// * `number_policy` - Synchronous number policy callback.
    ///
    /// # Returns
    ///
    /// The protected owned value.
    ///
    /// # Errors
    ///
    /// Returns decode, callback, exact-safe-integer, or validated-limit errors.
    pub fn decode_unique_validated_str<E, F>(
        &mut self,
        input: &str,
        options: &JsonValidatedDecodeOptions,
        number_policy: F,
    ) -> Result<SafeJsonValue, JsonValidatedDecodeError<E, R, Q>>
    where
        F: for<'call> FnMut(&'call [JsonPathSegment], JsonNumberToken<'call>) -> Result<JsonNumberPolicy, E>,
    {
        self.decode_unique_validated_utf8(input.as_bytes(), options, number_policy)
    }

    /// Reads a byte input through EOF and applies a source-number policy during
    /// decoding.
    ///
    /// Accepted input bytes remain charged if the callback or later validation
    /// fails; value charges commit only after the complete document succeeds.
    ///
    /// # Parameters
    ///
    /// * `input` - Byte input read through EOF.
    /// * `options` - Root, number model, and per-document token limits.
    /// * `number_policy` - Synchronous callback for each source number.
    ///
    /// # Returns
    ///
    /// The protected owned value.
    ///
    /// # Errors
    ///
    /// Returns structured decode, read, callback, exact-integer, or limit
    /// errors.
    pub fn decode_unique_validated_input<E, F, I>(
        &mut self,
        input: &mut I,
        options: &JsonValidatedDecodeOptions,
        number_policy: F,
    ) -> Result<SafeJsonValue, JsonValidatedDecodeError<E, R, Q>>
    where
        I: ByteInput + ?Sized,
        F: for<'call> FnMut(&'call [JsonPathSegment], JsonNumberToken<'call>) -> Result<JsonNumberPolicy, E>,
    {
        self.engine
            .decode_unique_validated_input(input, options, number_policy, self.diagnostic_policy)
    }

    /// Decodes one byte input using an explicit integer/fallback number model.
    ///
    /// The fallback model converts only integer tokens outside `i64`/`u64`
    /// to finite `f64`; decimal and exponent forms use the finite-float rule
    /// in both models.
    ///
    /// # Parameters
    ///
    /// * `input` - Byte input read through EOF.
    /// * `model` - Integer representation model for admitted source numbers.
    ///
    /// # Returns
    ///
    /// The owned value produced by Serde.
    ///
    /// # Errors
    ///
    /// Returns read, UTF-8, syntax, numeric-range, budget, or Serde errors.
    pub fn decode_input_with_number_model<T, I>(
        &mut self,
        input: &mut I,
        model: JsonDecodeNumberModel,
    ) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
        I: ByteInput + ?Sized,
    {
        let model = match model {
            JsonDecodeNumberModel::Integer64 => crate::lexical::JsonNumberModel::Integer64,
            JsonDecodeNumberModel::Integer64WithFiniteFallback => {
                crate::lexical::JsonNumberModel::Integer64WithFiniteFallback
            }
        };
        self.engine
            .decode_input_seed(TypedSeed::new(), input, self.diagnostic_policy, model)
    }

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

    /// Decodes a complete document from an item-oriented byte input through
    /// EOF.
    ///
    /// Without an input-byte limit, the input is read in fixed-size chunks.
    /// With a limit, each read is bounded by the remaining allowance plus at
    /// most one probe byte. The probe distinguishes an input ending exactly
    /// at the limit from an oversized input; it is neither charged nor
    /// buffered, and cannot be returned to the input source. The complete
    /// admitted document is buffered in memory because the returned value
    /// must not borrow from the temporary buffer. Without an input-byte
    /// limit, this buffering has no implicit size cap.
    ///
    /// # Parameters
    ///
    /// * `input` - Mutable byte input read through EOF.
    ///
    /// # Errors
    ///
    /// Returns a structured error for read failures, input-budget rejection,
    /// invalid UTF-8, invalid JSON, or target deserialization failures.
    pub fn decode_input<T, I>(&mut self, input: &mut I) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
        I: Input<Item = u8> + ?Sized,
    {
        self.engine.decode_input(input, self.diagnostic_policy)
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

    /// Decodes a unique-key JSON document into an owned target.
    ///
    /// The document is first materialized under the strict number contract,
    /// then exposed as native Serde events even with `arbitrary_precision`.
    /// Source construction uses the configured value depth budget and has no
    /// separate fixed container limit. Targets cannot borrow the input, and
    /// RawValue targets receive reconstructed JSON.
    ///
    /// # Errors
    ///
    /// Returns an input, syntax, budget, duplicate-key, construction-depth,
    /// or target-deserialization error. Value usage commits only on success.
    pub fn decode_unique_str<T>(&mut self, input: &str) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
    {
        self.decode_unique_utf8(input.as_bytes())
    }

    /// Decodes a unique-key UTF-8 JSON document into an owned target.
    ///
    /// See [`Self::decode_unique_str`] for numeric and materialization rules.
    ///
    /// # Errors
    ///
    /// Returns an input, UTF-8, syntax, budget, materialization, or target
    /// error.
    pub fn decode_unique_utf8<T>(&mut self, input: &[u8]) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: DeserializeOwned,
    {
        self.decode_unique_seed_utf8(TypedSeed::new(), input)
    }

    /// Decodes unique-key JSON through a seed producing an owned result.
    ///
    /// The seed receives native scalar, sequence, and map events from an
    /// intermediate strict JSON tree. Input and value budgets cover the source
    /// document. The temporary tree and target can coexist during decoding;
    /// these lexical budgets are not a precise heap-memory bound.
    /// Object iteration follows `serde_json::Map` ordering. The target's own
    /// Serde semantics still apply: use `DuplicateKeyRejectingJsonValue`
    /// instead of a bare `serde_json::Value` to preserve literal private keys.
    ///
    /// # Errors
    ///
    /// Returns a structured decode error for input admission, invalid syntax,
    /// duplicate keys, configured value-depth failure, or seed failure.
    /// Materialization errors retain source coordinates; target errors have
    /// no original source position and are redacted by default.
    pub fn decode_unique_seed_utf8<S, T>(&mut self, seed: S, input: &[u8]) -> Result<T, JsonDecodeError<R, Q>>
    where
        S: for<'de> DeserializeSeed<'de, Value = T>,
    {
        self.engine.decode_unique_seed_utf8(seed, input, self.diagnostic_policy)
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
    /// # Returns
    ///
    /// `Ok(())` after the complete document is valid and accounted for.
    ///
    /// # Errors
    ///
    /// Returns a structured error when accounting or JSON parsing fails. No
    /// target value is allocated; `str` input is already valid UTF-8.
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
    /// # Returns
    ///
    /// `Ok(())` after the complete document is valid and accounted for.
    ///
    /// # Errors
    ///
    /// Returns a structured error when accounting, UTF-8 validation, or JSON
    /// parsing fails. No target value is allocated.
    pub fn validate_utf8(&mut self, input: &[u8]) -> Result<(), JsonDecodeError<R, Q>> {
        self.engine.validate_utf8(input, self.diagnostic_policy)
    }
}

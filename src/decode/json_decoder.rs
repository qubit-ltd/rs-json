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
    /// Shared generic decoding and accounting core.
    engine: JsonDecodeEngine<'budget, R, Q>,
}

impl<R, Q> JsonDecoder<'static, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a decoder with a cumulative session built from explicit limits.
    #[inline(always)]
    #[must_use]
    pub fn with_limits(limits: JsonDecodeLimits<R, Q>) -> Self {
        Self::new(JsonDecodeSession::from_limits(limits))
    }
}

impl JsonDecoder<'static, JsonResource, usize> {
    /// Creates a decoder with no configured input or value limits.
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
    #[inline]
    #[must_use]
    pub const fn new(session: JsonDecodeSession<'budget, R, Q>) -> Self {
        Self {
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

    /// Decodes one complete JSON string and permits results borrowing `input`.
    pub fn decode_str<'de, T>(&mut self, input: &'de str) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.decode_seed_str(TypedSeed::new(), input)
    }

    /// Decodes one complete UTF-8 JSON byte slice and permits borrowed results.
    pub fn decode_utf8<'de, T>(&mut self, input: &'de [u8]) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.decode_seed_utf8(TypedSeed::new(), input)
    }

    /// Decodes a JSON string through a caller-provided Serde seed.
    pub fn decode_seed_str<'de, S>(&mut self, seed: S, input: &'de str) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        self.decode_seed_utf8(seed, input.as_bytes())
    }

    /// Decodes a UTF-8 byte slice through a caller-provided Serde seed.
    pub fn decode_seed_utf8<'de, S>(&mut self, seed: S, input: &'de [u8]) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        self.engine.decode_seed_utf8(seed, input, DiagnosticPolicy::Redacted)
    }

    /// Decodes a complete JSON string while requiring a top-level object.
    pub fn decode_object_str<'de, T>(&mut self, input: &'de str) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.decode_object_utf8(input.as_bytes())
    }

    /// Decodes a complete UTF-8 byte slice while requiring a top-level object.
    pub fn decode_object_utf8<'de, T>(&mut self, input: &'de [u8]) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.engine.decode_seed_utf8_with_top_level(
            TypedSeed::new(),
            input,
            DiagnosticPolicy::Redacted,
            Some(JsonRootKind::Object),
        )
    }

    /// Decodes a complete JSON string while requiring a top-level array.
    pub fn decode_array_str<'de, T>(&mut self, input: &'de str) -> Result<Vec<T>, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.decode_array_utf8(input.as_bytes())
    }

    /// Decodes a complete UTF-8 byte slice while requiring a top-level array.
    pub fn decode_array_utf8<'de, T>(&mut self, input: &'de [u8]) -> Result<Vec<T>, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.engine.decode_seed_utf8_with_top_level(
            TypedSeed::new(),
            input,
            DiagnosticPolicy::Redacted,
            Some(JsonRootKind::Array),
        )
    }

    /// Validates and accounts for one complete JSON string without
    /// materializing a target value.
    pub fn validate_str(&mut self, input: &str) -> Result<(), JsonDecodeError<R, Q>> {
        self.validate_utf8(input.as_bytes())
    }

    /// Validates and accounts for one complete UTF-8 JSON byte slice without
    /// materializing a target value.
    pub fn validate_utf8(&mut self, input: &[u8]) -> Result<(), JsonDecodeError<R, Q>> {
        self.engine.validate_utf8(input, DiagnosticPolicy::Redacted)
    }
}

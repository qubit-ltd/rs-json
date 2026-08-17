// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Stateful strict JSON text decoding with caller-owned resource accounting.

use std::marker::PhantomData;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use serde::Deserialize;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde_json::Deserializer as JsonDeserializer;

use super::JsonDecodeError;
use crate::lexical::JsonLexicalScanner;

/// Strictly decodes complete JSON documents while owning cumulative accounting
/// state.
#[derive(Debug)]
pub struct JsonDecoder<'budget, R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    session: JsonDecodeSession<'budget, R, Q>,
}

impl Default for JsonDecoder<'static, JsonResource, usize> {
    fn default() -> Self {
        Self::new(JsonDecodeSession::owned(JsonDecodeLimits::default()))
    }
}

#[allow(missing_docs)]
impl<'budget, R, Q> JsonDecoder<'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a decoder that owns a reusable cumulative session.
    ///
    /// # Parameters
    ///
    /// * `session` - Session whose budgets receive all input and value charges.
    ///
    /// # Returns
    ///
    /// A decoder that retains `session` until [`Self::into_session`] is called
    /// or the decoder is dropped.
    #[must_use]
    pub const fn new(session: JsonDecodeSession<'budget, R, Q>) -> Self {
        Self { session }
    }

    /// Returns the cumulative session for read-only inspection.
    #[must_use]
    pub const fn session(&self) -> &JsonDecodeSession<'budget, R, Q> {
        &self.session
    }

    /// Returns mutable access to the cumulative session.
    #[must_use]
    pub const fn session_mut(&mut self) -> &mut JsonDecodeSession<'budget, R, Q> {
        &mut self.session
    }

    /// Returns the cumulative session and consumes the decoder.
    #[must_use]
    pub fn into_session(self) -> JsonDecodeSession<'budget, R, Q> {
        self.session
    }

    /// Decodes one complete UTF-8 JSON string and accumulates its charges.
    pub fn decode_str<'de, T>(
        &mut self,
        input: &'de str,
    ) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
        R: Clone,
        Q: ResourceQuantity,
    {
        self.decode_seed_str(TypedSeed::new(), input)
    }

    /// Decodes one complete UTF-8 JSON byte slice and accumulates its charges.
    pub fn decode_utf8<'de, T>(
        &mut self,
        input: &'de [u8],
    ) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
        R: Clone,
        Q: ResourceQuantity,
    {
        self.decode_seed_utf8(TypedSeed::new(), input)
    }

    /// Decodes a string through a caller-provided Serde seed.
    pub fn decode_seed_str<'de, S>(
        &mut self,
        seed: S,
        input: &'de str,
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
        R: Clone,
        Q: ResourceQuantity,
    {
        self.decode_seed_utf8(seed, input.as_bytes())
    }

    /// Decodes a UTF-8 byte slice through a caller-provided Serde seed.
    pub fn decode_seed_utf8<'de, S>(
        &mut self,
        seed: S,
        input: &'de [u8],
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
        R: Clone,
        Q: ResourceQuantity,
    {
        decode_seed_impl(seed, input, &mut self.session)
    }

    /// Validates and accounts for one complete UTF-8 JSON string.
    pub fn validate_str(
        &mut self,
        input: &str,
    ) -> Result<(), JsonDecodeError<R, Q>>
    where
        R: Clone,
        Q: ResourceQuantity,
    {
        self.validate_utf8(input.as_bytes())
    }

    /// Validates and accounts for one complete UTF-8 JSON byte slice.
    pub fn validate_utf8(
        &mut self,
        input: &[u8],
    ) -> Result<(), JsonDecodeError<R, Q>>
    where
        R: Clone,
        Q: ResourceQuantity,
    {
        validate_impl(input, &mut self.session)
    }

    #[deprecated(note = "use decode_utf8")]
    pub fn decode<'de, T>(&mut self, input: &'de [u8]) -> Result<T, JsonDecodeError<R, Q>> where T: Deserialize<'de> { self.decode_utf8(input) }
    #[deprecated(note = "use decode_seed_utf8")]
    pub fn decode_seed<'de, S>(&mut self, seed: S, input: &'de [u8]) -> Result<S::Value, JsonDecodeError<R, Q>> where S: DeserializeSeed<'de> { self.decode_seed_utf8(seed, input) }
    #[deprecated(note = "use validate_utf8")]
    pub fn validate(&mut self, input: &[u8]) -> Result<(), JsonDecodeError<R, Q>> { self.validate_utf8(input) }
}

fn decode_seed_impl<'de, S, R, Q>(
    seed: S,
    input: &'de [u8],
    session: &mut JsonDecodeSession<'_, R, Q>,
) -> Result<S::Value, JsonDecodeError<R, Q>>
where
    S: DeserializeSeed<'de>,
    R: Clone,
    Q: ResourceQuantity,
{
    let mut attempt = session.begin_value();
    attempt
        .try_consume_input_bytes(input.len())
        .map_err(JsonDecodeError::Budget)?;
    JsonLexicalScanner::new(attempt.value_transaction_mut())
        .scan(input)
        .map_err(JsonDecodeError::from_lexical)?;
    let mut deserializer = JsonDeserializer::from_slice(input);
    let value = seed
        .deserialize(&mut deserializer)
        .map_err(|error| JsonDecodeError::from_serde(&error))?;
    deserializer
        .end()
        .map_err(|error| JsonDecodeError::from_serde(&error))?;
    attempt.commit();
    Ok(value)
}

fn validate_impl<R, Q>(
    input: &[u8],
    session: &mut JsonDecodeSession<'_, R, Q>,
) -> Result<(), JsonDecodeError<R, Q>>
where
    R: Clone,
    Q: ResourceQuantity,
{
    let mut attempt = session.begin_value();
    attempt
        .try_consume_input_bytes(input.len())
        .map_err(JsonDecodeError::Budget)?;
    JsonLexicalScanner::new(attempt.value_transaction_mut())
        .scan(input)
        .map_err(JsonDecodeError::from_lexical)?;
    attempt.commit();
    Ok(())
}

/// Seed adapter that delegates to [`Deserialize`] without allocating state.
struct TypedSeed<T> {
    marker: PhantomData<fn() -> T>,
}

impl<T> TypedSeed<T> {
    const fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

impl<'de, T> DeserializeSeed<'de> for TypedSeed<T>
where
    T: Deserialize<'de>,
{
    type Value = T;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer)
    }
}

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
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use serde::Deserialize;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde_json::Deserializer as JsonDeserializer;

use super::JsonDecodeError;
use crate::lexical::JsonLexicalScanner;

/// Strictly decodes complete JSON documents.
///
/// A mutable [`JsonDecodeSession`] can be supplied to each operation, so a
/// default decoder can be reused with multiple sessions and input documents.
/// [`Self::new`] also remains available for code that prefers a session-bound
/// decoder and the one-argument legacy-style methods.
#[derive(Debug)]
pub struct JsonDecoder<'session, 'budget, R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    session: Option<&'session mut JsonDecodeSession<'budget, R, Q>>,
}

impl<'session, 'budget, R, Q> Default for JsonDecoder<'session, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    fn default() -> Self {
        Self { session: None }
    }
}

impl<'session, 'budget, R, Q> JsonDecoder<'session, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a decoder bound to a reusable session.
    #[must_use]
    pub const fn new(
        session: &'session mut JsonDecodeSession<'budget, R, Q>,
    ) -> Self {
        Self {
            session: Some(session),
        }
    }

    /// Decodes one complete UTF-8 JSON string.
    pub fn decode_str<'de, T>(
        &self,
        input: &'de str,
        session: &mut JsonDecodeSession<'_, R, Q>,
    ) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
        R: Clone,
        Q: ResourceQuantity,
    {
        self.decode_seed_str(TypedSeed::new(), input, session)
    }

    /// Decodes one complete UTF-8 JSON byte slice.
    pub fn decode_utf8<'de, T>(
        &self,
        input: &'de [u8],
        session: &mut JsonDecodeSession<'_, R, Q>,
    ) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
        R: Clone,
        Q: ResourceQuantity,
    {
        self.decode_seed_utf8(TypedSeed::new(), input, session)
    }

    /// Decodes a string through a caller-provided Serde seed.
    pub fn decode_seed_str<'de, S>(
        &self,
        seed: S,
        input: &'de str,
        session: &mut JsonDecodeSession<'_, R, Q>,
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
        R: Clone,
        Q: ResourceQuantity,
    {
        self.decode_seed_utf8(seed, input.as_bytes(), session)
    }

    /// Decodes a UTF-8 byte slice through a caller-provided Serde seed.
    pub fn decode_seed_utf8<'de, S>(
        &self,
        seed: S,
        input: &'de [u8],
        session: &mut JsonDecodeSession<'_, R, Q>,
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
        R: Clone,
        Q: ResourceQuantity,
    {
        decode_seed_impl(seed, input, session)
    }

    /// Decodes one complete JSON byte slice using the session supplied to
    /// [`Self::new`].
    pub fn decode<'de, T>(
        &mut self,
        input: &'de [u8],
    ) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        let session = self
            .session
            .as_deref_mut()
            .expect("JsonDecoder::decode requires a decoder created with new");
        decode_seed_impl(TypedSeed::new(), input, session)
    }

    /// Decodes one complete JSON byte slice through a caller-provided seed.
    pub fn decode_seed<'de, S>(
        &mut self,
        seed: S,
        input: &'de [u8],
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        let session = self.session.as_deref_mut().expect(
            "JsonDecoder::decode_seed requires a decoder created with new",
        );
        decode_seed_impl(seed, input, session)
    }

    /// Validates and accounts for one complete UTF-8 JSON string.
    pub fn validate_str(
        &self,
        input: &str,
        session: &mut JsonDecodeSession<'_, R, Q>,
    ) -> Result<(), JsonDecodeError<R, Q>>
    where
        R: Clone,
        Q: ResourceQuantity,
    {
        self.validate_utf8(input.as_bytes(), session)
    }

    /// Validates and accounts for one complete UTF-8 JSON byte slice.
    pub fn validate_utf8(
        &self,
        input: &[u8],
        session: &mut JsonDecodeSession<'_, R, Q>,
    ) -> Result<(), JsonDecodeError<R, Q>>
    where
        R: Clone,
        Q: ResourceQuantity,
    {
        validate_impl(input, session)
    }

    /// Validates one complete JSON byte slice using the session supplied to
    /// [`Self::new`].
    pub fn validate(
        &mut self,
        input: &[u8],
    ) -> Result<(), JsonDecodeError<R, Q>> {
        let session = self.session.as_deref_mut().expect(
            "JsonDecoder::validate requires a decoder created with new",
        );
        validate_impl(input, session)
    }
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

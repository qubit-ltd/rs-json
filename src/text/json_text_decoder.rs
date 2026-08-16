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
use serde::Deserialize;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde_json::Deserializer as JsonDeserializer;

use super::JsonDecodeError;
use crate::internal::JsonLexicalScanner;

/// Strictly decodes JSON text while charging a reusable decode session.
///
/// Input charges are retained after every attempt. Decoded-value charges are
/// staged and commit only after complete lexical and typed decoding succeeds.
#[must_use = "a JSON text decoder must be used to decode or validate input"]
pub struct JsonTextDecoder<'session, 'budget, R, Q = usize>
where
    Q: ResourceQuantity,
{
    /// Reusable caller-owned accounting state.
    session: &'session mut JsonDecodeSession<'budget, R, Q>,
}

impl<'session, 'budget, R, Q> JsonTextDecoder<'session, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a decoder that charges `session` for each attempted document.
    ///
    /// # Parameters
    ///
    /// * `session` - Caller-owned session that receives input and committed
    ///   value accounting.
    ///
    /// # Returns
    ///
    /// A decoder borrowing `session` for its lifetime.
    pub const fn new(
        session: &'session mut JsonDecodeSession<'budget, R, Q>,
    ) -> Self {
        Self { session }
    }

    /// Decodes one complete JSON document into `T`.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Target type deserialized from the admitted JSON document.
    ///
    /// # Parameters
    ///
    /// * `input` - Complete JSON document to decode.
    ///
    /// # Returns
    ///
    /// The deserialized target value.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError::Budget`] when input or value accounting is
    /// rejected, [`JsonDecodeError::Syntax`] when `input` is not one complete
    /// JSON document, or [`JsonDecodeError::Deserialize`] when the admitted
    /// value cannot be decoded into `T`. Input charges remain after errors;
    /// staged value charges roll back.
    pub fn decode<'de, T>(
        &mut self,
        input: &'de [u8],
    ) -> Result<T, JsonDecodeError<R, Q>>
    where
        T: Deserialize<'de>,
    {
        self.decode_seed(TypedSeed::new(), input)
    }

    /// Decodes one complete JSON document through a caller-provided seed.
    ///
    /// # Type Parameters
    ///
    /// * `S` - Seed that controls construction of the decoded value.
    ///
    /// # Parameters
    ///
    /// * `seed` - Deserialization seed used for the admitted document.
    /// * `input` - Complete JSON document to decode.
    ///
    /// # Returns
    ///
    /// The value produced by `seed`.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError::Budget`] for resource rejection,
    /// [`JsonDecodeError::Syntax`] for lexical rejection, or
    /// [`JsonDecodeError::Deserialize`] when the seed or final deserializer
    /// state rejects the admitted document. Input charges remain after errors;
    /// staged value charges roll back.
    pub fn decode_seed<'de, S>(
        &mut self,
        seed: S,
        input: &'de [u8],
    ) -> Result<S::Value, JsonDecodeError<R, Q>>
    where
        S: DeserializeSeed<'de>,
    {
        let mut attempt = self.session.begin_value();
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

    /// Validates and accounts for one complete JSON document without decoding
    /// a target Rust value.
    ///
    /// # Parameters
    ///
    /// * `input` - Complete JSON document to validate and account.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the complete document is valid and admitted.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError::Budget`] when input or value accounting is
    /// rejected, or [`JsonDecodeError::Syntax`] when `input` is not one
    /// complete JSON document. Input charges remain after errors; admitted
    /// value charges commit only on success.
    pub fn validate(
        &mut self,
        input: &[u8],
    ) -> Result<(), JsonDecodeError<R, Q>> {
        let mut attempt = self.session.begin_value();
        attempt
            .try_consume_input_bytes(input.len())
            .map_err(JsonDecodeError::Budget)?;
        JsonLexicalScanner::new(attempt.value_transaction_mut())
            .scan(input)
            .map_err(JsonDecodeError::from_lexical)?;
        attempt.commit();
        Ok(())
    }
}

/// Seed adapter that delegates to [`Deserialize`] without allocating state.
struct TypedSeed<T> {
    /// Selects the decoded type without expressing ownership of `T`.
    marker: PhantomData<fn() -> T>,
}

impl<T> TypedSeed<T> {
    /// Creates an empty seed for `T`.
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

    /// Deserializes `T` through the supplied Serde deserializer.
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer)
    }
}

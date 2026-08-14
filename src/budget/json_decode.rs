// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Budget-admitting JSON slice deserialization.
// qubit-style: allow source-test-pair

use std::marker::PhantomData;

use qubit_budget::ResourceQuantity;
use serde::Deserialize;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde_json::Deserializer as JsonDeserializer;

use super::JsonDecodeSession;
use super::JsonSerdeError;
use super::internal::JsonLexicalPreflight;

/// Deserializes one admitted JSON slice into `T`.
///
/// The session consumes the full input before lexical validation. Typed Serde
/// decoding begins only after that validation has staged every JSON value.
///
/// # Parameters
///
/// * `input` - Complete JSON document bytes to decode.
/// * `session` - Mutable session charged once for input and decoded resources.
///
/// # Returns
///
/// The decoded value after lexical admission and typed deserialization.
///
/// # Errors
///
/// Returns [`JsonSerdeError::Budget`] when input or value resources exceed the
/// session limits. Returns [`JsonSerdeError::Json`] when the input is not one
/// valid JSON value or cannot deserialize as `T`. Input bytes remain consumed
/// after every attempt, while staged value resources are retained only after
/// complete typed deserialization succeeds.
pub fn decode_slice<'de, T, R, Q>(
    input: &'de [u8],
    session: &mut JsonDecodeSession<'_, R, Q>,
) -> Result<T, JsonSerdeError<R, Q>>
where
    T: Deserialize<'de>,
    R: Clone,
    Q: ResourceQuantity,
{
    decode_slice_seed(PhantomSeed::<T>::new(), input, session)
}

/// Deserializes one admitted JSON slice through a caller-provided seed.
///
/// The session consumes the full input before lexical validation. Typed Serde
/// decoding begins only after that validation has staged every JSON value.
///
/// # Parameters
///
/// * `seed` - Serde seed that drives typed decoding.
/// * `input` - Complete JSON document bytes to decode.
/// * `session` - Mutable session charged once for input and decoded resources.
///
/// # Returns
///
/// The value produced by `seed` after lexical admission and typed decoding.
///
/// # Errors
///
/// Returns [`JsonSerdeError::Budget`] when input or value resources exceed the
/// session limits. Returns [`JsonSerdeError::Json`] when the input is not one
/// valid JSON value or the seed rejects it. Input bytes remain consumed after
/// every attempt, while staged value resources are retained only when the seed
/// accepts one complete value.
pub fn decode_slice_seed<'de, S, R, Q>(
    seed: S,
    input: &'de [u8],
    session: &mut JsonDecodeSession<'_, R, Q>,
) -> Result<S::Value, JsonSerdeError<R, Q>>
where
    S: DeserializeSeed<'de>,
    R: Clone,
    Q: ResourceQuantity,
{
    let mut attempt = session.begin_value();
    attempt
        .try_consume_input_bytes(input.len())
        .map_err(JsonSerdeError::from)?;
    JsonLexicalPreflight::new(attempt.value_transaction_mut())
        .inspect(input)?;
    let mut deserializer = JsonDeserializer::from_slice(input);
    let value = seed
        .deserialize(&mut deserializer)
        .map_err(JsonSerdeError::Json)?;
    deserializer.end().map_err(JsonSerdeError::Json)?;
    attempt.commit();
    Ok(value)
}

/// Seed adapter that delegates to [`Deserialize`] without allocating state.
struct PhantomSeed<T> {
    /// The decoded type selected by the caller.
    marker: PhantomData<T>,
}

impl<T> PhantomSeed<T> {
    /// Creates an empty seed for `T`.
    const fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

impl<'de, T> DeserializeSeed<'de> for PhantomSeed<T>
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

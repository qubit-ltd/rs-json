// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Adapts strict JSON decoding to operation-specific errors.

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonDecodeSession;
use serde::Deserialize;
use serde::de::DeserializeSeed;

use super::JsonDecodeError;
use super::JsonDeserializeError;
use crate::budget::JsonSerdeError;
use crate::budget::decode_slice as decode_slice_legacy;
use crate::budget::decode_slice_seed as decode_slice_seed_legacy;
use crate::budget::internal::JsonLexicalPreflight;

/// Decodes one strict JSON document into `T` while charging `session`.
pub fn decode_slice<'de, T, R, Q>(
    input: &'de [u8],
    session: &mut JsonDecodeSession<'_, R, Q>,
) -> Result<T, JsonDecodeError<R, Q>>
where
    T: Deserialize<'de>,
    R: Clone,
    Q: ResourceQuantity,
{
    decode_slice_legacy(input, session).map_err(map_error)
}

/// Validates one strict JSON document, retaining its input charge and
/// committing value resources only after complete lexical admission.
pub fn inspect<R, Q>(
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
    JsonLexicalPreflight::new(attempt.value_transaction_mut())
        .inspect(input)
        .map_err(map_error)?;
    attempt.commit();
    Ok(())
}

/// Decodes one strict JSON document through a caller-supplied Serde seed.
pub fn decode_slice_seed<'de, S, R, Q>(
    seed: S,
    input: &'de [u8],
    session: &mut JsonDecodeSession<'_, R, Q>,
) -> Result<S::Value, JsonDecodeError<R, Q>>
where
    S: DeserializeSeed<'de>,
    R: Clone,
    Q: ResourceQuantity,
{
    decode_slice_seed_legacy(seed, input, session).map_err(map_error)
}

/// Converts the legacy unified error into a decode-specific error.
fn map_error<R, Q>(error: JsonSerdeError<R, Q>) -> JsonDecodeError<R, Q>
where
    Q: ResourceQuantity,
{
    match error {
        JsonSerdeError::Budget(error) => JsonDecodeError::Budget(error.into()),
        JsonSerdeError::Quantity { resource, source } => {
            JsonDecodeError::Budget(MeasuredBudgetError::quantity(
                resource, source,
            ))
        }
        JsonSerdeError::Syntax(error) => JsonDecodeError::Syntax(error),
        JsonSerdeError::Json(error) => JsonDecodeError::Deserialize(
            JsonDeserializeError::from_serde(&error),
        ),
        JsonSerdeError::Io(_) => {
            JsonDecodeError::Deserialize(JsonDeserializeError::IO)
        }
    }
}

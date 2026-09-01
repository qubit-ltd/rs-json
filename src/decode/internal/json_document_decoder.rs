// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared lexical admission and Serde materialization for JSON documents.

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonDecodeAttempt;
use serde::de::DeserializeSeed;
use serde_json::Deserializer as JsonDeserializer;
use serde_json::Error;

use crate::lexical::JsonLexicalError;
use crate::lexical::JsonLexicalScanner;

/// Lexically validates and stages value accounting for one complete document.
///
/// # Parameters
///
/// * `attempt` - Decode attempt whose value transaction receives staged usage.
/// * `input` - Complete JSON document bytes to scan.
/// * `has_value_limits` - Whether value measurements can reject the document.
///
/// # Errors
///
/// Returns a syntax failure or the first value-budget rejection reported by
/// the lexical scanner. All successful measurements remain staged in
/// `attempt` until its caller commits or drops the attempt.
pub(in crate::decode) fn admit_json_document<R, Q>(
    attempt: &mut JsonDecodeAttempt<'_, R, Q>,
    input: &[u8],
    has_value_limits: bool,
) -> Result<(), JsonLexicalError<R, Q>>
where
    R: Clone,
    Q: ResourceQuantity,
{
    JsonLexicalScanner::new(attempt.value_transaction_mut(), has_value_limits).scan(input)
}

/// Deserializes one complete JSON document through a caller-provided seed.
///
/// # Parameters
///
/// * `seed` - Serde seed that materializes the target value.
/// * `input` - Complete JSON document bytes borrowed for `'de`.
///
/// # Returns
///
/// The value produced by `seed` after the deserializer confirms that no input
/// remains after the document.
///
/// # Errors
///
/// Returns [`Error`] when the seed rejects the target shape or the
/// deserializer does not consume one complete document.
///
/// # Panics
///
/// Panics raised by the seed's visitor propagate to the caller.
pub(in crate::decode) fn deserialize_json_document<'de, S>(
    seed: S,
    input: &'de [u8],
) -> Result<S::Value, Error>
where
    S: DeserializeSeed<'de>,
{
    let mut deserializer = JsonDeserializer::from_slice(input);
    let value = seed.deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}

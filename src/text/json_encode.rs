// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Adapts strict JSON encoding to operation-specific errors.

use std::io::Write;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonEncodeSession;
use serde::Serialize;
use serde_json::Error as JsonError;

use crate::budget::JsonSerdeError;
use crate::budget::encode_to_vec as encode_to_vec_legacy;
use crate::budget::encode_to_writer as encode_to_writer_legacy;

use super::JsonEncodeError;

/// Encodes `value` into compact JSON bytes while charging `session`.
pub fn encode_to_vec<T, R, Q>(
    value: &T,
    session: &mut JsonEncodeSession<'_, R, Q>,
) -> Result<Vec<u8>, JsonEncodeError<R, Q>>
where
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    encode_to_vec_legacy(value, session).map_err(map_error)
}

/// Encodes `value` and writes it after successful serialization and accounting.
pub fn encode_to_writer<W, T, R, Q>(
    writer: W,
    value: &T,
    session: &mut JsonEncodeSession<'_, R, Q>,
) -> Result<(), JsonEncodeError<R, Q>>
where
    W: Write,
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    encode_to_writer_legacy(writer, value, session).map_err(map_error)
}

/// Converts the legacy unified error into an encode-specific error.
fn map_error<R, Q>(error: JsonSerdeError<R, Q>) -> JsonEncodeError<R, Q>
where
    Q: ResourceQuantity,
{
    match error {
        JsonSerdeError::Budget(error) => JsonEncodeError::Budget(error.into()),
        JsonSerdeError::Quantity { resource, source } => {
            JsonEncodeError::Budget(MeasuredBudgetError::quantity(resource, source))
        }
        JsonSerdeError::Syntax(error) => {
            JsonEncodeError::InvalidRawJson(JsonError::io(std::io::Error::other(error.to_string())))
        }
        JsonSerdeError::Json(error) => JsonEncodeError::Serialize(error),
        JsonSerdeError::Io(error) => JsonEncodeError::Write(error),
    }
}

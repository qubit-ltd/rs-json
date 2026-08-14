// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Budget-aware JSON encoding APIs.
// qubit-style: allow source-test-pair

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use serde::Serialize;
use serde_json::Serializer as JsonSerializer;

use super::JsonEncodeSession;
use super::JsonSerdeError;
use super::internal::JsonEncodeSerializer;
use super::internal::JsonOutputAccounting;
use super::internal::JsonOutputBuffer;
use super::internal::JsonOutputWriter;

/// Serializes one value to a compact JSON vector while charging its output.
///
/// # Parameters
///
/// * `value` - Value serialized into compact JSON.
/// * `session` - Mutable JSON session charged before delegation and output
///   growth.
///
/// # Returns
///
/// Compact JSON bytes when serialization and every budget check succeed.
/// Output bytes are committed to the caller's session only after the complete
/// document succeeds.
///
/// # Errors
///
/// Returns [`JsonSerdeError::Json`] when serialization fails, or
/// [`JsonSerdeError::Budget`] when an output or structural limit is exceeded.
///
/// # Type Parameters
///
/// * `T` - Value type serialized to JSON.
/// * `R` - Resource identity reported by budget violations.
pub fn encode_to_vec<T, R, Q>(
    value: &T,
    session: &mut JsonEncodeSession<R, Q>,
) -> Result<Vec<u8>, JsonSerdeError<R, Q>>
where
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    let mut transaction = session.output_budget().cloned();
    let initial_remaining =
        transaction.as_ref().map(|budget| budget.remaining());
    let bytes = {
        let accounting = Rc::new(RefCell::new(JsonOutputAccounting::new(
            transaction.as_mut(),
        )));
        let mut output = JsonOutputBuffer::new(Rc::clone(&accounting));
        let result = {
            let mut inner = JsonSerializer::new(&mut output);
            let context = RefCell::new(super::internal::JsonEncodeContext {
                budget: session.value_budget_mut(),
                output: accounting,
            });
            value.serialize(JsonEncodeSerializer::new(&mut inner, &context))
        };
        output.into_result(result)?
    };
    if let (Some(transaction), Some(initial_remaining)) =
        (transaction, initial_remaining)
    {
        let consumed = initial_remaining - transaction.remaining();
        session
            .consume_output_bytes(consumed)
            .map_err(MeasuredBudgetError::from)
            .map_err(JsonSerdeError::from)?;
    }
    Ok(bytes)
}

/// Serializes one value and writes it only after budget checks pass.
///
/// Serialization is transactional with respect to budget and Serde failures:
/// the destination is not touched until the complete buffered document is
/// accepted. A failure during the final [`Write::write_all`] call may leave the
/// destination with a partial document because [`Write`] has no rollback API.
///
/// # Parameters
///
/// * `writer` - Destination that receives the compact JSON bytes.
/// * `value` - Value serialized into compact JSON.
/// * `session` - Mutable JSON session charged for output and value resources.
///
/// # Returns
///
/// `Ok(())` when serialization, budget checks, and the write all succeed.
///
/// # Errors
///
/// Returns [`JsonSerdeError::Json`] or [`JsonSerdeError::Budget`] from
/// [`encode_to_vec`], or [`JsonSerdeError::Io`] when the writer fails.
///
/// # Type Parameters
///
/// * `W` - Writer that accepts the serialized JSON bytes.
/// * `T` - Value type serialized to JSON.
/// * `R` - Resource identity reported by budget violations.
pub fn encode_to_writer<W, T, R, Q>(
    mut writer: W,
    value: &T,
    session: &mut JsonEncodeSession<R, Q>,
) -> Result<(), JsonSerdeError<R, Q>>
where
    W: Write,
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    let bytes = encode_to_vec(value, session)?;
    writer.write_all(&bytes).map_err(JsonSerdeError::Io)
}

/// Serializes one value directly to a writer with online budget checks.
///
/// Unlike [`encode_to_writer`], this function permits accepted prefixes to
/// remain in `writer` when Serde, budget, or I/O processing fails. Output
/// budget consumption is committed for every byte accepted before failure.
///
/// # Parameters
///
/// * `writer` - Destination receiving accepted JSON bytes.
/// * `value` - Value serialized into compact JSON.
/// * `session` - Mutable session charged for value and output resources.
///
/// # Returns
///
/// `Ok(())` when serialization and every online budget and writer operation
/// succeeds.
///
/// # Errors
///
/// Returns [`JsonSerdeError::Budget`] when a configured limit is exceeded,
/// [`JsonSerdeError::Json`] for a Serde failure, or [`JsonSerdeError::Io`]
/// when the destination rejects bytes. Accepted prefixes and charges remain
/// visible after any error.
///
/// # Type Parameters
///
/// * `W` - Destination writer type.
/// * `T` - Serialized value type.
/// * `R` - Resource identity reported by budget failures.
pub fn encode_to_writer_incremental<W, T, R, Q>(
    writer: W,
    value: &T,
    session: &mut JsonEncodeSession<R, Q>,
) -> Result<(), JsonSerdeError<R, Q>>
where
    W: Write,
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    let mut transaction = session.output_budget().cloned();
    let initial_remaining =
        transaction.as_ref().map(|budget| budget.remaining());
    let result = {
        let accounting = Rc::new(RefCell::new(JsonOutputAccounting::new(
            transaction.as_mut(),
        )));
        let mut output = JsonOutputWriter::new(writer, Rc::clone(&accounting));
        let result = {
            let mut inner = JsonSerializer::new(&mut output);
            let context = RefCell::new(super::internal::JsonEncodeContext {
                budget: session.value_budget_mut(),
                output: accounting,
            });
            value.serialize(JsonEncodeSerializer::new(&mut inner, &context))
        };
        output.into_result(result)
    };
    if let (Some(transaction), Some(initial_remaining)) =
        (transaction, initial_remaining)
    {
        let consumed = initial_remaining - transaction.remaining();
        session
            .consume_output_bytes(consumed)
            .map_err(MeasuredBudgetError::from)
            .map_err(JsonSerdeError::from)?;
    }
    result
}

// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Stateful strict JSON text encoding.
// qubit-style: allow source-test-pair

use std::cell::RefCell;
use std::fmt::Debug;
use std::io::Write;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonEncodeAttempt;
use qubit_budget::json::JsonEncodeSession;
use serde::Serialize;
use serde_json::Error as JsonError;
use serde_json::Serializer as JsonSerializer;

use super::JsonEncodeError;
use super::internal::JsonEncodeSerializer;
use super::internal::JsonOutputAccounting;
use super::internal::JsonOutputBuffer;
use super::internal::JsonOutputWriter;

/// Encodes strict JSON text while charging a caller-managed session.
pub struct JsonTextEncoder<'session, 'budget, R, Q = usize>
where
    Q: ResourceQuantity,
{
    /// Session charged by each encode operation.
    session: &'session mut JsonEncodeSession<'budget, R, Q>,
}

impl<'session, 'budget, R, Q> JsonTextEncoder<'session, 'budget, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// Creates an encoder borrowing `session` for its lifetime.
    ///
    /// # Parameters
    ///
    /// * `session` - Caller-owned session that receives committed output
    ///   accounting.
    ///
    /// # Returns
    ///
    /// An encoder borrowing `session` for its lifetime.
    pub fn new(
        session: &'session mut JsonEncodeSession<'budget, R, Q>,
    ) -> Self {
        Self { session }
    }

    /// Encodes `value` into compact JSON and commits only complete success.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Source type serialized into JSON.
    ///
    /// # Parameters
    ///
    /// * `value` - Value to serialize.
    ///
    /// # Returns
    ///
    /// The compact JSON bytes on success.
    ///
    /// # Errors
    ///
    /// Returns [`JsonEncodeError::Budget`] when accounting rejects the value
    /// or output, or a serialization error when Serde rejects `value`.
    pub fn to_vec<T>(
        &mut self,
        value: &T,
    ) -> Result<Vec<u8>, JsonEncodeError<R, Q>>
    where
        T: Serialize + ?Sized,
    {
        let mut attempt = self.session.begin_value();
        let bytes = Self::serialize_buffer(value, &mut attempt)?;
        attempt
            .check_output_bytes(bytes.len())
            .map_err(JsonEncodeError::Budget)?;
        attempt
            .try_consume_output_bytes(bytes.len())
            .map_err(JsonEncodeError::Budget)?;
        attempt.commit();
        Ok(bytes)
    }

    /// Buffers a complete document before writing it to `writer`.
    ///
    /// # Type Parameters
    ///
    /// * `W` - Destination writer type.
    /// * `T` - Source type serialized into JSON.
    ///
    /// # Parameters
    ///
    /// * `writer` - Destination receiving the complete JSON document.
    /// * `value` - Value to serialize.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the complete document is written and accounting is
    /// committed.
    ///
    /// # Errors
    ///
    /// Returns [`JsonEncodeError::Budget`] when accounting rejects the value
    /// or output, or a serialization/writer error on failure.
    pub fn write_buffered<W, T>(
        &mut self,
        writer: W,
        value: &T,
    ) -> Result<(), JsonEncodeError<R, Q>>
    where
        W: Write,
        T: Serialize + ?Sized,
    {
        let mut attempt = self.session.begin_value();
        let bytes = Self::serialize_buffer(value, &mut attempt)?;
        attempt
            .check_output_bytes(bytes.len())
            .map_err(JsonEncodeError::Budget)?;
        Self::write_buffer(writer, &bytes, &mut attempt)?;
        attempt.commit();
        Ok(())
    }

    /// Streams `value` directly to `writer`, retaining accepted prefixes.
    ///
    /// # Type Parameters
    ///
    /// * `W` - Destination writer type.
    /// * `T` - Source type serialized into JSON.
    ///
    /// # Parameters
    ///
    /// * `writer` - Destination receiving streamed JSON bytes.
    /// * `value` - Value to serialize.
    ///
    /// # Returns
    ///
    /// `Ok(())` after serialization and output accounting complete.
    ///
    /// # Errors
    ///
    /// Returns [`JsonEncodeError::Budget`] when accounting rejects output,
    /// or a serialization/writer error on failure. Accepted output prefixes
    /// remain written when a later operation fails.
    pub fn write_incremental<W, T>(
        &mut self,
        writer: W,
        value: &T,
    ) -> Result<(), JsonEncodeError<R, Q>>
    where
        W: Write,
        T: Serialize + ?Sized,
    {
        let mut attempt = self.session.begin_value();
        let result = {
            let (output_budget, transaction) = attempt.split_mut();
            let accounting =
                RefCell::new(JsonOutputAccounting::new(output_budget));
            let mut output = JsonOutputWriter::new(writer, &accounting);
            let result = {
                let mut inner = JsonSerializer::new(&mut output);
                let context =
                    RefCell::new(super::internal::JsonEncodeContext {
                        transaction,
                        output: &accounting,
                    });
                value.serialize(JsonEncodeSerializer::new(&mut inner, &context))
            };
            if result.is_ok() {
                let _ = output.flush();
            }
            output.into_result(result)
        };
        result?;
        attempt.commit();
        Ok(())
    }

    /// Serializes one value into an output-bounded byte buffer.
    fn serialize_buffer<T>(
        value: &T,
        attempt: &mut JsonEncodeAttempt<'_, R, Q>,
    ) -> Result<Vec<u8>, JsonEncodeError<R, Q>>
    where
        T: Serialize + ?Sized,
    {
        let (output_budget, transaction) = attempt.split_mut();
        let accounting = RefCell::new(JsonOutputAccounting::new(output_budget));
        let mut output = JsonOutputBuffer::new(&accounting);
        let result = {
            let mut inner = JsonSerializer::new(&mut output);
            let context = RefCell::new(super::internal::JsonEncodeContext {
                transaction,
                output: &accounting,
            });
            value.serialize(JsonEncodeSerializer::new(&mut inner, &context))
        };
        if result.is_ok() {
            let _ = output.flush();
        }
        output.into_result(result)
    }

    /// Writes buffered bytes and charges each accepted prefix.
    fn write_buffer<W>(
        writer: W,
        bytes: &[u8],
        attempt: &mut JsonEncodeAttempt<'_, R, Q>,
    ) -> Result<(), JsonEncodeError<R, Q>>
    where
        W: Write,
    {
        let (output_budget, _) = attempt.split_mut();
        let accounting = RefCell::new(JsonOutputAccounting::new(output_budget));
        let mut output = JsonOutputWriter::new(writer, &accounting);
        let result = output.write_all(bytes).map_err(JsonError::io);
        output.into_result(result)
    }
}

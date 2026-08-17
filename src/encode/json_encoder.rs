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
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use serde::Serialize;
use serde_json::Error as JsonError;
use serde_json::Serializer as JsonSerializer;

use super::JsonEncodeError;
use super::output::JsonOutputAccounting;
use super::output::JsonOutputBuffer;
use super::output::JsonOutputWriter;
use super::serializer::json_encode_context::JsonEncodeContext;
use super::serializer::json_encode_serializer::JsonEncodeSerializer;

/// Encodes strict JSON text while owning cumulative accounting state.
pub struct JsonEncoder<'budget, R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    /// Session charged by each encode operation.
    session: JsonEncodeSession<'budget, R, Q>,
}

impl Default for JsonEncoder<'static, JsonResource, usize> {
    fn default() -> Self {
        Self::new(JsonEncodeSession::owned(JsonEncodeLimits::default()))
    }
}

impl<'budget, R, Q> JsonEncoder<'budget, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// Creates an encoder that owns a reusable cumulative session.
    ///
    /// # Parameters
    ///
    /// * `session` - Session that receives committed output accounting.
    ///
    /// # Returns
    ///
    /// An encoder that retains `session` until [`Self::into_session`] is called
    /// or the encoder is dropped.
    #[inline(always)]
    #[must_use]
    pub fn new(session: JsonEncodeSession<'budget, R, Q>) -> Self {
        Self { session }
    }

    /// Returns the cumulative session for read-only inspection.
    #[must_use]
    pub const fn session(&self) -> &JsonEncodeSession<'budget, R, Q> {
        &self.session
    }

    /// Returns mutable access to the cumulative session.
    #[must_use]
    pub const fn session_mut(&mut self) -> &mut JsonEncodeSession<'budget, R, Q> {
        &mut self.session
    }

    /// Returns the cumulative session and consumes the encoder.
    #[must_use]
    pub fn into_session(self) -> JsonEncodeSession<'budget, R, Q> {
        self.session
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
                let context = RefCell::new(JsonEncodeContext {
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
            let context = RefCell::new(JsonEncodeContext {
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

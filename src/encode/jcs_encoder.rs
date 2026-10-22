// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Budget-aware encoder for JSON Canonicalization Scheme (JCS) output.

use std::cell::RefCell;
use std::fmt::Debug;
use std::io::Write;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonEncodeAttempt;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_io::Output;
use serde_json::Value;

use super::JcsEncodeError;
use super::JsonSerializationError;
use super::JsonSerializationErrorKind;
use super::internal::JcsRenderer;
use super::internal::JcsRendererError;
use super::output::JsonOutputAccounting;
use super::output::JsonOutputBuffer;
use super::output::JsonOutputWriter;
use crate::value::traverse::JsonTreeReader;

/// Encodes [`serde_json::Value`] values using RFC 8785 canonical JSON.
///
/// JCS uses an IEEE 754 binary64 number model. Integer values outside the
/// exactly representable range can therefore be rounded (for example,
/// `9007199254740993` becomes `9007199254740992`). Output limits measure the
/// final canonical byte sequence. Auxiliary storage used while rendering is
/// limited separately by `max_working_bytes`; this limit covers the traversal
/// stack and active object member tables, but excludes the input [`Value`] and
/// buffered output bytes.
///
/// # Examples
///
/// ```
/// use qubit_json::encode::JcsEncoder;
///
/// let mut encoder = JcsEncoder::unlimited();
/// let bytes = encoder.to_vec(&serde_json::json!({"b": 2, "a": 1.0}))?;
/// assert_eq!(bytes, br#"{"a":1,"b":2}"#);
/// # Ok::<(), qubit_json::encode::JcsEncodeError<qubit_budget::json::JsonResource>>(())
/// ```
#[must_use]
pub struct JcsEncoder<'budget, R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    /// Cumulative accounting state shared by encode operations.
    session: JsonEncodeSession<'budget, R, Q>,
    /// Maximum auxiliary memory used while rendering a value.
    max_working_bytes: usize,
}

impl<R, Q> JcsEncoder<'static, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// Creates an encoder with an owned session using `limits`.
    ///
    /// `max_working_bytes` caps the capacity of the renderer's traversal stack
    /// and active object member tables.
    pub fn with_limits(limits: JsonEncodeLimits<R, Q>, max_working_bytes: usize) -> Self {
        Self::new(JsonEncodeSession::from_limits(limits), max_working_bytes)
    }
}

impl JcsEncoder<'static, JsonResource, usize> {
    /// Creates an encoder with effectively unlimited value, output, and
    /// working memory limits.
    pub fn unlimited() -> Self {
        Self::with_limits(JsonEncodeLimits::new(), usize::MAX)
    }
}

impl<'budget, R, Q> JcsEncoder<'budget, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// Creates an encoder that retains the supplied cumulative session.
    ///
    /// `max_working_bytes` caps the capacity of the renderer's traversal stack
    /// and active object member tables.
    pub fn new(session: JsonEncodeSession<'budget, R, Q>, max_working_bytes: usize) -> Self {
        Self {
            session,
            max_working_bytes,
        }
    }

    /// Returns the maximum auxiliary working memory in bytes.
    #[must_use]
    #[inline]
    pub const fn max_working_bytes(&self) -> usize {
        self.max_working_bytes
    }

    /// Returns the cumulative accounting session.
    #[must_use]
    #[inline]
    pub const fn session(&self) -> &JsonEncodeSession<'budget, R, Q> {
        &self.session
    }

    /// Returns mutable access to the cumulative accounting session.
    #[must_use]
    #[inline]
    pub const fn session_mut(&mut self) -> &mut JsonEncodeSession<'budget, R, Q> {
        &mut self.session
    }

    /// Consumes the encoder and returns its cumulative accounting session.
    #[must_use]
    pub fn into_session(self) -> JsonEncodeSession<'budget, R, Q> {
        self.session
    }

    /// Encodes a value and commits its accounting only after full success.
    pub fn to_vec(&mut self, value: &Value) -> Result<Vec<u8>, JcsEncodeError<R, Q>> {
        let mut attempt = self.session.begin_value();
        let bytes = Self::serialize_buffer(value, &mut attempt, self.max_working_bytes)?;
        attempt
            .try_consume_output_bytes(bytes.len())
            .map_err(JcsEncodeError::budget)?;
        attempt.commit().map_err(JcsEncodeError::budget)?;
        Ok(bytes)
    }

    /// Encodes a value as UTF-8 text, committing only after successful
    /// encoding.
    pub fn to_string(&mut self, value: &Value) -> Result<String, JcsEncodeError<R, Q>> {
        let bytes = self.to_vec(value)?;
        Ok(String::from_utf8(bytes).expect("JCS output must be UTF-8"))
    }

    /// Buffers the canonical document before writing it to `output`.
    ///
    /// Output budget is checked against the complete document before external
    /// I/O. If the destination fails, accepted bytes remain written and remain
    /// charged, while the value transaction is not committed.
    pub fn write_buffered<O: Output<Item = u8>>(
        &mut self,
        output: O,
        value: &Value,
    ) -> Result<(), JcsEncodeError<R, Q>> {
        let mut attempt = self.session.begin_value();
        let bytes = Self::serialize_buffer(value, &mut attempt, self.max_working_bytes)?;
        attempt
            .check_output_bytes(bytes.len())
            .map_err(JcsEncodeError::budget)?;
        Self::write_buffer(output, &bytes, &mut attempt)?;
        attempt.commit().map_err(JcsEncodeError::budget)
    }

    /// Writes canonical output directly to `output` as it is produced.
    ///
    /// Accepted bytes are charged immediately and remain written if rendering,
    /// flushing, or destination I/O later fails. A failed operation does not
    /// commit the value budget.
    pub fn write_incremental<O: Output<Item = u8>>(
        &mut self,
        output: O,
        value: &Value,
    ) -> Result<(), JcsEncodeError<R, Q>> {
        let mut attempt = self.session.begin_value();
        JsonTreeReader::new(attempt.value_transaction_mut())
            .account(value)
            .map_err(JcsEncodeError::budget)?;
        let result = {
            let (output_budget, _) = attempt.split_mut();
            let accounting = RefCell::new(JsonOutputAccounting::new(output_budget));
            let mut writer = JsonOutputWriter::new(output, &accounting);
            match JcsRenderer::render(value, &mut writer, self.max_working_bytes) {
                Ok(()) => {
                    let result = Write::flush(&mut writer).map_err(serde_json::Error::io);
                    writer
                        .into_result(result)
                        .map_err(JcsEncodeError::from_json_encode)
                }
                Err(JcsRendererError::WorkingMemory(error)) => {
                    Err(JcsEncodeError::working_memory(error))
                }
                Err(JcsRendererError::Allocation) => Err(JcsEncodeError::serialization(
                    JsonSerializationError::new(JsonSerializationErrorKind::CustomSerialization),
                )),
                Err(JcsRendererError::Serialize(error)) => writer
                    .into_result(Err(error))
                    .map_err(JcsEncodeError::from_json_encode),
            }
        };
        result?;
        attempt.commit().map_err(JcsEncodeError::budget)
    }

    /// Produces a complete canonical document in an output-bounded buffer.
    fn serialize_buffer(
        value: &Value,
        attempt: &mut JsonEncodeAttempt<'_, R, Q>,
        max_working_bytes: usize,
    ) -> Result<Vec<u8>, JcsEncodeError<R, Q>> {
        JsonTreeReader::new(attempt.value_transaction_mut())
            .account(value)
            .map_err(JcsEncodeError::budget)?;
        let (output_budget, _) = attempt.split_mut();
        let accounting = RefCell::new(JsonOutputAccounting::new(output_budget));
        let mut output = JsonOutputBuffer::new(&accounting);
        match JcsRenderer::render(value, &mut output, max_working_bytes) {
            Ok(()) => output
                .into_result(Ok(()))
                .map_err(JcsEncodeError::from_json_encode),
            Err(JcsRendererError::WorkingMemory(error)) => {
                Err(JcsEncodeError::working_memory(error))
            }
            Err(JcsRendererError::Allocation) => Err(JcsEncodeError::serialization(
                JsonSerializationError::new(JsonSerializationErrorKind::CustomSerialization),
            )),
            Err(JcsRendererError::Serialize(error)) => output
                .into_result(Err(error))
                .map_err(JcsEncodeError::from_json_encode),
        }
    }

    /// Writes buffered bytes and charges destination-accepted prefixes.
    fn write_buffer<O: Output<Item = u8>>(
        output: O,
        bytes: &[u8],
        attempt: &mut JsonEncodeAttempt<'_, R, Q>,
    ) -> Result<(), JcsEncodeError<R, Q>> {
        let (output_budget, _) = attempt.split_mut();
        let accounting = RefCell::new(JsonOutputAccounting::new(output_budget));
        let mut writer = JsonOutputWriter::new(output, &accounting);
        let result = writer.write_all(bytes).map_err(serde_json::Error::io);
        writer
            .into_result(result)
            .map_err(JcsEncodeError::from_json_encode)
    }
}

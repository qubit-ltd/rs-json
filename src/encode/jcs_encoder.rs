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
use qubit_io::ByteOutput;
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
/// limited separately by `max_working_bytes`; this limit covers the renderer's
/// traversal stack and active object member tables, but excludes the input
/// [`Value`], buffered output bytes, and any admission stack used to check
/// configured value limits before rendering. Configure `max_depth` as well
/// when the admission traversal itself must have a depth bound. A working
/// memory limit is a capacity bound for these renderer collections, not an
/// exact heap-usage bound.
///
/// # Type Parameters
///
/// * `'budget` - Lifetime of the resource budget borrowed by the encoder
///   session.
/// * `R` - Resource identity type tracked by the session.
/// * `Q` - Quantity type used for resource limits and accounting.
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
    ///
    /// # Type Parameters
    ///
    /// * `R` - Resource identity type tracked by the session.
    /// * `Q` - Quantity type used for resource limits and accounting.
    ///
    /// # Parameters
    ///
    /// * `limits` supplies cumulative value and output limits.
    /// * `max_working_bytes` limits auxiliary traversal and member-table
    ///   memory.
    ///
    /// # Returns
    ///
    /// An encoder that owns a session initialized from `limits`.
    pub fn with_limits(limits: JsonEncodeLimits<R, Q>, max_working_bytes: usize) -> Self {
        Self::new(JsonEncodeSession::from_limits(limits), max_working_bytes)
    }
}

impl JcsEncoder<'static, JsonResource, usize> {
    /// Creates an encoder with effectively unlimited value, output, and
    /// working memory limits.
    ///
    /// # Returns
    ///
    /// An encoder backed by a fresh session without configured resource limits.
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
    ///
    /// # Type Parameters
    ///
    /// * `'budget` - Lifetime of the resource budget borrowed by `session`.
    /// * `R` - Resource identity type tracked by `session`.
    /// * `Q` - Quantity type used for resource limits and accounting.
    ///
    /// # Parameters
    ///
    /// * `session` is the cumulative accounting session to retain.
    /// * `max_working_bytes` limits auxiliary traversal and member-table
    ///   memory.
    ///
    /// # Returns
    ///
    /// An encoder using the supplied session and working-memory limit.
    pub fn new(session: JsonEncodeSession<'budget, R, Q>, max_working_bytes: usize) -> Self {
        Self {
            session,
            max_working_bytes,
        }
    }

    /// Returns the maximum auxiliary working memory in bytes.
    ///
    /// # Returns
    ///
    /// The configured maximum working-memory capacity.
    #[must_use]
    #[inline]
    pub const fn max_working_bytes(&self) -> usize {
        self.max_working_bytes
    }

    /// Returns the cumulative accounting session.
    ///
    /// # Returns
    ///
    /// A shared reference to this encoder's session.
    #[must_use]
    #[inline]
    pub const fn session(&self) -> &JsonEncodeSession<'budget, R, Q> {
        &self.session
    }

    /// Returns mutable access to the cumulative accounting session.
    ///
    /// # Returns
    ///
    /// A mutable reference to this encoder's session.
    #[must_use]
    #[inline]
    pub const fn session_mut(&mut self) -> &mut JsonEncodeSession<'budget, R, Q> {
        &mut self.session
    }

    /// Consumes the encoder and returns its cumulative accounting session.
    ///
    /// # Returns
    ///
    /// The session containing the cumulative resource accounting state.
    #[must_use]
    pub fn into_session(self) -> JsonEncodeSession<'budget, R, Q> {
        self.session
    }

    /// Encodes a value and commits its accounting only after full success.
    ///
    /// # Parameters
    ///
    /// * `value` is the JSON value to encode.
    ///
    /// # Returns
    ///
    /// The complete canonical JSON bytes on success.
    ///
    /// # Errors
    ///
    /// Returns `JcsEncodeError` if rendering, memory allocation, or resource
    /// accounting fails. The value and output accounting is committed only on
    /// success.
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
    ///
    /// # Parameters
    ///
    /// * `value` is the JSON value to encode.
    ///
    /// # Returns
    ///
    /// The complete canonical JSON document as a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns `JcsEncodeError` if encoding or resource accounting fails.
    ///
    /// # Panics
    ///
    /// Panics only if the renderer produces bytes that are not valid UTF-8,
    /// which violates the encoder's JSON output invariant.
    pub fn to_string(&mut self, value: &Value) -> Result<String, JcsEncodeError<R, Q>> {
        let bytes = self.to_vec(value)?;
        Ok(String::from_utf8(bytes).expect("JCS output must be UTF-8"))
    }

    /// Buffers the canonical document before writing it to `output`.
    ///
    /// Output budget is checked against the complete document before external
    /// I/O. If the destination fails, accepted bytes remain written and remain
    /// charged, while the value transaction is not committed.
    /// This method does not flush the destination. Success means that the
    /// destination accepted the complete document and the encoding charges
    /// were committed. The writer is borrowed mutably for this call; flush it
    /// explicitly afterward. A later flush is outside this encoding transaction
    /// and does not roll back committed charges if it fails. Do not rely on
    /// `Drop` to report flush errors. A successful flush does not guarantee
    /// durability or delivery to a remote peer.
    ///
    /// # Type Parameters
    ///
    /// * `O` - Output destination receiving the complete buffered document.
    ///
    /// # Parameters
    ///
    /// * `output` is borrowed mutably and receives the complete document.
    /// * `value` is the JSON value to encode.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the destination has accepted all bytes and accounting
    /// is committed. The destination is not flushed.
    ///
    /// # Errors
    ///
    /// Returns `JcsEncodeError` if rendering, output budgeting, destination
    /// I/O, or session accounting fails. A destination failure preserves
    /// accepted output bytes and their output charges without committing the
    /// value transaction. A later caller-initiated flush failure does not undo
    /// the already committed charges.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::BufWriter;
    /// use std::io::Write;
    ///
    /// use qubit_budget::json::JsonEncodeLimits;
    /// use qubit_budget::json::JsonResource;
    /// use qubit_json::encode::JcsEncoder;
    /// use serde_json::json;
    ///
    /// let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
    ///     .max_output_bytes(64)
    ///     .build();
    /// let mut encoder = JcsEncoder::with_limits(limits, 16 * 1024);
    /// let mut writer = BufWriter::new(Vec::new());
    /// encoder.write_buffered(&mut writer, &json!({"b": 2, "a": 1}))?;
    /// writer.flush()?;
    /// let bytes = writer.into_inner().expect("flush already succeeded");
    /// assert_eq!(bytes, br#"{"a":1,"b":2}"#);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn write_buffered<O: ByteOutput + ?Sized>(
        &mut self,
        output: &mut O,
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
    /// commit the value budget. On success, this method flushes the destination
    /// before committing value accounting; a flush failure therefore retains
    /// charges for accepted output bytes without committing the value budget.
    /// The destination is borrowed mutably for this call.
    ///
    /// # Type Parameters
    ///
    /// * `O` - Output destination receiving bytes as they are rendered.
    ///
    /// # Parameters
    ///
    /// * `output` is borrowed mutably and receives bytes as they are rendered.
    /// * `value` is the JSON value to encode.
    ///
    /// # Returns
    ///
    /// `Ok(())` after rendering, flushing, and accounting all succeed.
    ///
    /// # Errors
    ///
    /// Returns `JcsEncodeError` if rendering, working-memory allocation,
    /// destination I/O, or session accounting fails. Accepted output bytes
    /// remain written and charged, while failed operations do not commit the
    /// value budget.
    pub fn write_incremental<O: ByteOutput + ?Sized>(
        &mut self,
        output: &mut O,
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
                    writer.into_result(result).map_err(JcsEncodeError::from_json_encode)
                }
                Err(JcsRendererError::WorkingMemory(error)) => Err(JcsEncodeError::working_memory(error)),
                Err(JcsRendererError::Number(source)) => Err(JcsEncodeError::serialization(source)),
                Err(JcsRendererError::Allocation) => Err(JcsEncodeError::serialization(JsonSerializationError::new(
                    JsonSerializationErrorKind::CustomSerialization,
                ))),
                Err(JcsRendererError::Serialize(error)) => {
                    writer.into_result(Err(error)).map_err(JcsEncodeError::from_json_encode)
                }
            }
        };
        result?;
        attempt.commit().map_err(JcsEncodeError::budget)
    }

    /// Produces a complete canonical document in an output-bounded buffer.
    ///
    /// # Parameters
    ///
    /// * `value` - JSON value whose canonical representation is produced.
    /// * `attempt` - In-progress value transaction that accounts the value.
    /// * `max_working_bytes` - Maximum auxiliary memory used by the renderer.
    ///
    /// # Returns
    ///
    /// The complete canonical JSON bytes.
    ///
    /// # Errors
    ///
    /// Returns a JCS error if value accounting, rendering, output budgeting,
    /// or working-memory allocation fails.
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
            Ok(()) => output.into_result(Ok(())).map_err(JcsEncodeError::from_json_encode),
            Err(JcsRendererError::WorkingMemory(error)) => Err(JcsEncodeError::working_memory(error)),
            Err(JcsRendererError::Number(source)) => Err(JcsEncodeError::serialization(source)),
            Err(JcsRendererError::Allocation) => Err(JcsEncodeError::serialization(JsonSerializationError::new(
                JsonSerializationErrorKind::CustomSerialization,
            ))),
            Err(JcsRendererError::Serialize(error)) => {
                output.into_result(Err(error)).map_err(JcsEncodeError::from_json_encode)
            }
        }
    }

    /// Writes buffered bytes and charges destination-accepted prefixes.
    ///
    /// # Type Parameters
    ///
    /// * `O` - Output destination that accepts the buffered bytes.
    ///
    /// # Parameters
    ///
    /// * `output` - Destination receiving the buffered document.
    /// * `bytes` - Complete document bytes to write.
    /// * `attempt` - In-progress transaction whose output budget is charged.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after all bytes are accepted by the destination.
    ///
    /// # Errors
    ///
    /// Returns a JCS error if destination writing or output accounting fails.
    fn write_buffer<O: ByteOutput + ?Sized>(
        output: &mut O,
        bytes: &[u8],
        attempt: &mut JsonEncodeAttempt<'_, R, Q>,
    ) -> Result<(), JcsEncodeError<R, Q>> {
        let (output_budget, _) = attempt.split_mut();
        let accounting = RefCell::new(JsonOutputAccounting::new(output_budget));
        let mut writer = JsonOutputWriter::new(output, &accounting);
        let result = writer.write_all(bytes).map_err(serde_json::Error::io);
        writer.into_result(result).map_err(JcsEncodeError::from_json_encode)
    }
}

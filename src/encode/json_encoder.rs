// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Stateful strict JSON text encoding.

use std::cell::RefCell;
use std::fmt::Debug;
use std::io::Write;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonEncodeAttempt;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_io::ByteOutput;
use serde::Serialize;
use serde_json::Error as JsonError;
use serde_json::Serializer as JsonSerializer;

use super::JsonEncodeError;
use super::output::JsonOutputAccounting;
use super::output::JsonOutputBuffer;
use super::output::JsonOutputWriter;
use super::serializer::JsonEncodeContext;
use super::serializer::JsonEncodeSerializer;

/// Encodes strict JSON text while owning cumulative accounting state.
///
/// Signed and unsigned 64-bit integers are supported in full. Serde `i128`
/// values are accepted only when they fit `i64`, or are non-negative and fit
/// `u64`; `u128` values must fit `u64`. Wider integers return a serialization
/// error instead of being truncated or converted to strings. Floating-point
/// values must be finite.
///
/// Nested serialization errors remain failures of the complete operation even
/// when an enclosing custom `Serialize` implementation ignores them. Arbitrary
/// custom error text is not retained. Incremental output may already contain an
/// accepted prefix when an error is reported.
///
/// # Type Parameters
///
/// * `'budget` - Lifetime of the caller-provided cumulative encode session.
/// * `R` - Resource identity tracked by the encode session.
/// * `Q` - Quantity representation used for resource accounting.
///
/// # Examples
///
/// ```
/// use qubit_json::encode::JsonEncoder;
///
/// let mut encoder = JsonEncoder::unlimited();
/// let bytes = encoder.to_vec(&serde_json::json!({"ok": true}))?;
/// assert_eq!(bytes, br#"{"ok":true}"#);
/// # Ok::<(), qubit_json::encode::JsonEncodeError<qubit_budget::json::JsonResource>>(())
/// ```
pub struct JsonEncoder<'budget, R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    /// Session charged by each encode operation.
    session: JsonEncodeSession<'budget, R, Q>,
}

impl<R, Q> JsonEncoder<'static, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// Creates an encoder with an owned session built from explicit limits.
    ///
    /// # Parameters
    ///
    /// * `limits` - Resource limits used to construct the cumulative session.
    ///
    /// # Returns
    ///
    /// An encoder whose cumulative accounting starts empty and is constrained
    /// by `limits`.
    #[inline(always)]
    #[must_use]
    pub fn with_limits(limits: JsonEncodeLimits<R, Q>) -> Self {
        Self::new(JsonEncodeSession::from_limits(limits))
    }
}

impl JsonEncoder<'static, JsonResource, usize> {
    /// Creates an encoder with an explicitly unlimited standard session.
    ///
    /// # Returns
    ///
    /// An encoder with no configured output or encoded-value limits.
    #[inline(always)]
    #[must_use]
    pub fn unlimited() -> Self {
        Self::with_limits(JsonEncodeLimits::new())
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
    ///
    /// The reference exposes cumulative accounting state. Bytes accepted by
    /// an external destination remain charged even when the encode operation
    /// fails; value-resource charges commit only after the complete operation
    /// succeeds. The reference remains borrowed from this encoder.
    ///
    /// # Returns
    ///
    /// A shared reference to the cumulative encode session.
    #[inline(always)]
    #[must_use]
    pub const fn session(&self) -> &JsonEncodeSession<'budget, R, Q> {
        &self.session
    }

    /// Returns mutable access to the cumulative session.
    ///
    /// Changes made through the reference affect the limits and accounting
    /// state used by subsequent encode operations.
    ///
    /// # Returns
    ///
    /// A mutable reference to the cumulative encode session.
    #[inline(always)]
    #[must_use]
    pub const fn session_mut(&mut self) -> &mut JsonEncodeSession<'budget, R, Q> {
        &mut self.session
    }

    /// Returns the cumulative session and consumes the encoder.
    ///
    /// No output is produced and no accounting is reset; ownership of the
    /// accumulated state is transferred to the caller.
    ///
    /// # Returns
    ///
    /// The session previously owned by this encoder.
    #[inline(always)]
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
    /// Returns [`JsonEncodeErrorKind::Budget`](super::JsonEncodeErrorKind::Budget) when accounting rejects the value
    /// or output,
    /// [`JsonEncodeErrorKind::InvalidRawJson`](super::JsonEncodeErrorKind::InvalidRawJson)
    /// when a `RawValue` or private raw payload fails this encoder's strict
    /// JSON admission, or a serialization error when Serde rejects `value`.
    /// Inspect [`JsonEncodeError::syntax_error`] for the structured syntax
    /// reason.
    pub fn to_vec<T>(&mut self, value: &T) -> Result<Vec<u8>, JsonEncodeError<R, Q>>
    where
        T: Serialize + ?Sized,
    {
        let has_value_limits = self.session.value_budget().limits().has_limits();
        let mut attempt = self.session.begin_value();
        let bytes = Self::serialize_buffer(value, &mut attempt, has_value_limits)?;
        attempt
            .try_consume_output_bytes(bytes.len())
            .map_err(JsonEncodeError::<R, Q>::budget)?;
        attempt.commit().map_err(JsonEncodeError::<R, Q>::budget)?;
        Ok(bytes)
    }

    /// Buffers a complete document before writing it to `writer`.
    ///
    /// Success means `writer` accepted the complete document and output/value
    /// accounting was committed. This method does not flush the writer. If
    /// the writer buffers output, call its [`Write::flush`] method after this
    /// call succeeds. The writer is borrowed mutably for this call. A later
    /// flush failure is outside this encoding transaction and does not roll
    /// back accounting already committed by this method. A successful flush
    /// only reports the writer's result; it does not confirm persistence to
    /// disk or receipt by a remote peer.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::{BufWriter, Write};
    /// use qubit_budget::json::{JsonEncodeLimits, JsonResource};
    /// use qubit_json::encode::JsonEncoder;
    /// use serde_json::json;
    ///
    /// let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
    ///     .max_output_bytes(64)
    ///     .build();
    /// let mut encoder = JsonEncoder::with_limits(limits);
    /// let mut writer = BufWriter::new(Vec::new());
    /// encoder.write_buffered(&mut writer, &json!({"ok": true}))?;
    /// writer.flush()?;
    /// let bytes = writer.into_inner().expect("flush already succeeded");
    /// assert_eq!(bytes, br#"{"ok":true}"#);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Type Parameters
    ///
    /// * `O` - Destination output type.
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
    /// Returns [`JsonEncodeErrorKind::Budget`](super::JsonEncodeErrorKind::Budget) when accounting rejects the value
    /// or output,
    /// [`JsonEncodeErrorKind::InvalidRawJson`](super::JsonEncodeErrorKind::InvalidRawJson)
    /// when a `RawValue` or private raw payload fails this encoder's strict
    /// JSON admission, a serialization error when Serde rejects `value`, or
    /// a writer error when writing fails. Inspect
    /// [`JsonEncodeError::syntax_error`] for the structured syntax reason.
    pub fn write_buffered<O, T>(&mut self, writer: &mut O, value: &T) -> Result<(), JsonEncodeError<R, Q>>
    where
        O: ByteOutput + ?Sized,
        T: Serialize + ?Sized,
    {
        let has_value_limits = self.session.value_budget().limits().has_limits();
        let mut attempt = self.session.begin_value();
        let bytes = Self::serialize_buffer(value, &mut attempt, has_value_limits)?;
        attempt
            .check_output_bytes(bytes.len())
            .map_err(JsonEncodeError::<R, Q>::budget)?;
        Self::write_buffer(writer, &bytes, &mut attempt)?;
        attempt.commit().map_err(JsonEncodeError::<R, Q>::budget)?;
        Ok(())
    }

    /// Streams `value` directly to `writer`, retaining accepted prefixes.
    ///
    /// After serialization succeeds, this method flushes `writer` before
    /// committing value-resource accounting. A flush failure is reported as a
    /// writer error, and bytes already accepted remain charged to the output
    /// budget while value-resource accounting is not committed. A successful
    /// flush only reports the writer's result; it does not confirm persistence
    /// to disk or receipt by a remote peer. The writer is borrowed mutably for
    /// this call. Do not rely on `Drop` to flush buffered output.
    ///
    /// # Type Parameters
    ///
    /// * `O` - Destination output type.
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
    /// Returns [`JsonEncodeErrorKind::Budget`](super::JsonEncodeErrorKind::Budget)
    /// when accounting rejects value resources or output bytes,
    /// [`JsonEncodeErrorKind::InvalidRawJson`](super::JsonEncodeErrorKind::InvalidRawJson) when a
    /// `RawValue` or private raw payload fails this encoder's strict JSON
    /// admission, a serialization error when Serde rejects `value`, or a
    /// writer error when writing or flushing fails. Inspect
    /// [`JsonEncodeError::syntax_error`] for the structured syntax reason.
    /// Accepted output prefixes remain written and charged when a later
    /// operation fails; value-resource charges commit only after the complete
    /// operation succeeds.
    pub fn write_incremental<O, T>(&mut self, writer: &mut O, value: &T) -> Result<(), JsonEncodeError<R, Q>>
    where
        O: ByteOutput + ?Sized,
        T: Serialize + ?Sized,
    {
        let has_value_limits = self.session.value_budget().limits().has_limits();
        let mut attempt = self.session.begin_value();
        let result = {
            let (output_budget, transaction) = attempt.split_mut();
            let accounting = RefCell::new(JsonOutputAccounting::new(output_budget));
            let mut output = JsonOutputWriter::new(writer, &accounting);
            let result = {
                let mut inner = JsonSerializer::new(&mut output);
                let context = RefCell::new(JsonEncodeContext {
                    transaction,
                    output: &accounting,
                    has_value_limits,
                });
                if has_value_limits {
                    value.serialize(JsonEncodeSerializer::<_, R, Q, true>::new(&mut inner, &context))
                } else {
                    value.serialize(JsonEncodeSerializer::<_, R, Q, false>::new(&mut inner, &context))
                }
            };
            if result.is_ok() {
                let _ = Write::flush(&mut output);
            }
            output.into_result(result)
        };
        result?;
        attempt.commit().map_err(JsonEncodeError::<R, Q>::budget)?;
        Ok(())
    }

    /// Serializes one value into an output-bounded byte buffer.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Source type serialized into JSON.
    ///
    /// # Parameters
    ///
    /// * `value` - Value to serialize.
    /// * `attempt` - Encode attempt receiving staged value and output charges.
    /// * `has_value_limits` - Whether serialized values have active limits.
    ///
    /// # Returns
    ///
    /// The serialized JSON bytes after all applicable output checks pass.
    ///
    /// # Errors
    ///
    /// Returns a budget error when value or output accounting rejects the
    /// serialization, an invalid-raw-JSON error for malformed private raw
    /// values, or a serialization error from Serde.
    fn serialize_buffer<T>(
        value: &T,
        attempt: &mut JsonEncodeAttempt<'_, R, Q>,
        has_value_limits: bool,
    ) -> Result<Vec<u8>, JsonEncodeError<R, Q>>
    where
        T: Serialize + ?Sized,
    {
        let (output_budget, transaction) = attempt.split_mut();
        if output_budget.is_none() {
            let accounting = RefCell::new(JsonOutputAccounting::new(None));
            let mut bytes = Vec::new();
            let result = {
                let mut inner = JsonSerializer::new(&mut bytes);
                let context = RefCell::new(JsonEncodeContext {
                    transaction,
                    output: &accounting,
                    has_value_limits,
                });
                if has_value_limits {
                    value.serialize(JsonEncodeSerializer::<_, R, Q, true>::new(&mut inner, &context))
                } else {
                    value.serialize(JsonEncodeSerializer::<_, R, Q, false>::new(&mut inner, &context))
                }
            };
            if let Some(error) = accounting.borrow_mut().take_violation() {
                return Err(JsonEncodeError::<R, Q>::budget(error));
            }
            if let Some(error) = accounting.borrow_mut().take_syntax_error() {
                return Err(JsonEncodeError::<R, Q>::invalid_raw_json(error));
            }
            if let Some(error) = accounting.borrow_mut().take_serialization_error() {
                return Err(JsonEncodeError::<R, Q>::serialization(error));
            }
            if result.is_err() {
                let error = accounting.borrow_mut().take_serialization_error_or_custom();
                return Err(JsonEncodeError::<R, Q>::serialization(error));
            }
            return Ok(bytes);
        }
        let accounting = RefCell::new(JsonOutputAccounting::new(output_budget));
        let mut output = JsonOutputBuffer::new(&accounting);
        let result = {
            let mut inner = JsonSerializer::new(&mut output);
            let context = RefCell::new(JsonEncodeContext {
                transaction,
                output: &accounting,
                has_value_limits,
            });
            if has_value_limits {
                value.serialize(JsonEncodeSerializer::<_, R, Q, true>::new(&mut inner, &context))
            } else {
                value.serialize(JsonEncodeSerializer::<_, R, Q, false>::new(&mut inner, &context))
            }
        };
        if result.is_ok() {
            let _ = Write::flush(&mut output);
        }
        output.into_result(result)
    }

    /// Writes buffered bytes and charges each accepted prefix.
    ///
    /// # Type Parameters
    ///
    /// * `O` - Destination output type.
    ///
    /// # Parameters
    ///
    /// * `writer` - Destination receiving the buffered JSON bytes.
    /// * `bytes` - Complete serialized document to write.
    /// * `attempt` - Encode attempt receiving output charges.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after all bytes are written and accounted for.
    ///
    /// # Errors
    ///
    /// Returns a budget error if output accounting rejects a written prefix,
    /// or a writer error if the destination cannot accept the bytes.
    fn write_buffer<O>(
        writer: &mut O,
        bytes: &[u8],
        attempt: &mut JsonEncodeAttempt<'_, R, Q>,
    ) -> Result<(), JsonEncodeError<R, Q>>
    where
        O: ByteOutput + ?Sized,
    {
        let (output_budget, _) = attempt.split_mut();
        let accounting = RefCell::new(JsonOutputAccounting::new(output_budget));
        let mut output = JsonOutputWriter::new(writer, &accounting);
        let result = output.write_all(bytes).map_err(JsonError::io);
        output.into_result(result)
    }
}

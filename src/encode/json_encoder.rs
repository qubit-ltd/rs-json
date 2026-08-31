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
///
/// Signed and unsigned 64-bit integers are supported in full. Serde `i128`
/// values are accepted only when they fit `i64`, or are non-negative and fit
/// `u64`; `u128` values must fit `u64`. Wider integers return a serialization
/// error instead of being truncated or converted to strings. Floating-point
/// values must be finite.
///
/// # Type Parameters
///
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
    /// The reference exposes charges committed by completed encode
    /// operations and remains borrowed from this encoder.
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
    /// Returns [`JsonEncodeError::Budget`] when accounting rejects the value
    /// or output, or a serialization error when Serde rejects `value`.
    pub fn to_vec<T>(&mut self, value: &T) -> Result<Vec<u8>, JsonEncodeError<R, Q>>
    where
        T: Serialize + ?Sized,
    {
        let has_value_limits = self.session.value_budget().limits().has_limits();
        let mut attempt = self.session.begin_value();
        let bytes = Self::serialize_buffer(value, &mut attempt, has_value_limits)?;
        attempt
            .check_output_bytes(bytes.len())
            .map_err(JsonEncodeError::Budget)?;
        attempt
            .try_consume_output_bytes(bytes.len())
            .map_err(JsonEncodeError::Budget)?;
        attempt.commit().map_err(JsonEncodeError::Budget)?;
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
    pub fn write_buffered<W, T>(&mut self, writer: W, value: &T) -> Result<(), JsonEncodeError<R, Q>>
    where
        W: Write,
        T: Serialize + ?Sized,
    {
        let has_value_limits = self.session.value_budget().limits().has_limits();
        let mut attempt = self.session.begin_value();
        let bytes = Self::serialize_buffer(value, &mut attempt, has_value_limits)?;
        attempt
            .check_output_bytes(bytes.len())
            .map_err(JsonEncodeError::Budget)?;
        Self::write_buffer(writer, &bytes, &mut attempt)?;
        attempt.commit().map_err(JsonEncodeError::Budget)?;
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
    pub fn write_incremental<W, T>(&mut self, writer: W, value: &T) -> Result<(), JsonEncodeError<R, Q>>
    where
        W: Write,
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
                let _ = output.flush();
            }
            output.into_result(result)
        };
        result?;
        attempt.commit().map_err(JsonEncodeError::Budget)?;
        Ok(())
    }

    /// Serializes one value into an output-bounded byte buffer.
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
                return Err(JsonEncodeError::Budget(error));
            }
            if let Some(error) = accounting.borrow_mut().take_syntax_error() {
                return Err(JsonEncodeError::InvalidRawJson(error));
            }
            result.map_err(JsonEncodeError::Serialize)?;
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

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
use super::output::JsonOutputAccounting;
use super::output::JsonOutputBuffer;
use super::output::JsonOutputWriter;
use crate::value::traverse::JsonTreeReader;

/// Encodes [`serde_json::Value`] values using RFC 8785 canonical JSON.
///
/// JCS uses an IEEE 754 binary64 number model. Integer values outside the
/// exactly representable range can therefore be rounded (for example,
/// `9007199254740993` becomes `9007199254740992`). Output limits measure the
/// final canonical byte sequence. The canonicalizer may allocate temporary
/// storage while sorting object members; that storage is not covered by the
/// output byte budget.
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
}

impl<R, Q> JcsEncoder<'static, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// Creates an encoder with an owned session using `limits`.
    pub fn with_limits(limits: JsonEncodeLimits<R, Q>) -> Self {
        Self::new(JsonEncodeSession::from_limits(limits))
    }
}

impl JcsEncoder<'static, JsonResource, usize> {
    /// Creates an encoder with no configured resource limits.
    pub fn unlimited() -> Self {
        Self::with_limits(JsonEncodeLimits::new())
    }
}

impl<'budget, R, Q> JcsEncoder<'budget, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// Creates an encoder that retains the supplied cumulative session.
    pub fn new(session: JsonEncodeSession<'budget, R, Q>) -> Self {
        Self { session }
    }

    /// Returns the cumulative accounting session.
    #[must_use]
    pub const fn session(&self) -> &JsonEncodeSession<'budget, R, Q> {
        &self.session
    }

    /// Returns mutable access to the cumulative accounting session.
    #[must_use]
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
        let bytes = Self::serialize_buffer(value, &mut attempt)?;
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
        let bytes = Self::serialize_buffer(value, &mut attempt)?;
        attempt
            .check_output_bytes(bytes.len())
            .map_err(JcsEncodeError::budget)?;
        Self::write_buffer(output, &bytes, &mut attempt)?;
        attempt.commit().map_err(JcsEncodeError::budget)
    }

    /// Writes canonical output directly to `output` as it is produced.
    ///
    /// The canonicalizer can defer output while sorting object members. Any
    /// bytes accepted by the destination are charged immediately and remain
    /// written if a later serialization or I/O failure occurs.
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
            let result = serde_json_canonicalizer::to_writer(value, &mut writer);
            if result.is_ok() {
                let _ = Write::flush(&mut writer);
            }
            writer.into_result(result)
        };
        result.map_err(JcsEncodeError::from_json_encode)?;
        attempt.commit().map_err(JcsEncodeError::budget)
    }

    /// Produces a complete canonical document in an output-bounded buffer.
    fn serialize_buffer(
        value: &Value,
        attempt: &mut JsonEncodeAttempt<'_, R, Q>,
    ) -> Result<Vec<u8>, JcsEncodeError<R, Q>> {
        JsonTreeReader::new(attempt.value_transaction_mut())
            .account(value)
            .map_err(JcsEncodeError::budget)?;
        let (output_budget, _) = attempt.split_mut();
        let accounting = RefCell::new(JsonOutputAccounting::new(output_budget));
        let mut output = JsonOutputBuffer::new(&accounting);
        let result = serde_json_canonicalizer::to_writer(value, &mut output);
        output.into_result(result).map_err(JcsEncodeError::from_json_encode)
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
        writer.into_result(result).map_err(JcsEncodeError::from_json_encode)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::JcsEncoder;

    struct OutputOnly(Rc<RefCell<Vec<u8>>>);

    impl qubit_io::Output for OutputOnly {
        type Item = u8;

        unsafe fn write_unchecked(&mut self, bytes: &[u8], index: usize, count: usize) -> std::io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(&bytes[index..index + count]);
            Ok(count)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn canonicalizes_object() {
        let mut encoder = JcsEncoder::unlimited();
        let value = serde_json::json!({"b": 2, "a": 1.0});
        assert_eq!(encoder.to_vec(&value).unwrap(), br#"{"a":1,"b":2}"#);
    }

    #[test]
    fn writes_to_output_only_destination() {
        let value = serde_json::json!({"b": 2, "a": 1});
        let mut buffered_encoder = JcsEncoder::unlimited();
        let buffered = Rc::new(RefCell::new(Vec::new()));
        buffered_encoder
            .write_buffered(OutputOnly(buffered.clone()), &value)
            .unwrap();
        assert_eq!(*buffered.borrow(), br#"{"a":1,"b":2}"#);

        let mut incremental_encoder = JcsEncoder::unlimited();
        let incremental = Rc::new(RefCell::new(Vec::new()));
        incremental_encoder
            .write_incremental(OutputOnly(incremental.clone()), &value)
            .unwrap();
        assert_eq!(*incremental.borrow(), br#"{"a":1,"b":2}"#);
    }
}

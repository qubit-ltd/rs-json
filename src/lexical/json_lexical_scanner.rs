// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Non-recursive lexical admission for one JSON input document.

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonValueTransaction;

use super::JsonLexicalAdmissionPolicy;
use super::internal::JsonLexicalContainerFrame;
use super::internal::json_lexical_cursor::JsonLexicalCursor;
use super::json_lexical_consumer::JsonLexicalConsumer;
use super::json_lexical_consumer::JsonLexicalScanPoint;
use super::json_lexical_error::JsonLexicalError;
use super::json_lexical_error_reason::JsonLexicalErrorReason;
use super::json_lexical_event::JsonLexicalEvent;
use super::json_lexical_scan_error::JsonLexicalScanError;
use super::json_number_model::JsonNumberModel;
use crate::decode::JsonDecodeContext;

/// Lexically validates and charges one JSON document without recursion.
///
/// # Type Parameters
///
/// * `R` - Resource identity type used by the value transaction.
/// * `Q` - Quantity representation used for resource accounting.
#[must_use]
pub(crate) struct JsonLexicalScanner<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// JSON value resources charged while scanning the document.
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    /// Root-inclusive depth assigned to the scanned value.
    root_depth: usize,
    /// Whether scanning must stage JSON value measurements.
    has_value_limits: bool,
    /// Numeric range contract selected by the owning decoder.
    number_model: JsonNumberModel,
    /// Determines whether token admission uses delimiter-first or token-first
    /// checks.
    admission_policy: JsonLexicalAdmissionPolicy,
    /// Optional maximum root-inclusive depth; `None` adds no depth cap.
    depth_limit: Option<Q>,
    /// Optional maximum encoded bytes inside each string token, excluding
    /// quotes.
    max_encoded_string_bytes: Option<usize>,
}

impl<'transaction, 'budget, R, Q> JsonLexicalScanner<'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a lexical scanner bound to one value transaction.
    ///
    /// # Parameters
    ///
    /// * `transaction` - Value transaction charged for admitted measurements.
    /// * `has_value_limits` - Whether scanning stages value measurements.
    ///
    /// # Returns
    ///
    /// A scanner rooted at depth one using the `Integer64` number model.
    #[inline(always)]
    pub(crate) const fn new(
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
        has_value_limits: bool,
    ) -> Self {
        Self {
            transaction,
            root_depth: 1,
            has_value_limits,
            number_model: JsonNumberModel::Integer64,
            admission_policy: JsonLexicalAdmissionPolicy::Immediate,
            depth_limit: None,
            max_encoded_string_bytes: None,
        }
    }

    /// Creates a lexical scanner rooted at an enclosing serializer depth.
    ///
    /// # Parameters
    ///
    /// * `transaction` - Value transaction charged for admitted measurements.
    /// * `root_depth` - Root-inclusive depth assigned to the scanned value.
    /// * `has_value_limits` - Whether scanning stages value measurements.
    ///
    /// # Returns
    ///
    /// A scanner using the specified root depth and the `Integer64` model.
    #[inline(always)]
    pub(crate) const fn at_depth(
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
        root_depth: usize,
        has_value_limits: bool,
    ) -> Self {
        Self {
            transaction,
            root_depth,
            has_value_limits,
            number_model: JsonNumberModel::Integer64,
            admission_policy: JsonLexicalAdmissionPolicy::Immediate,
            depth_limit: None,
            max_encoded_string_bytes: None,
        }
    }

    /// Selects the number range contract for this scan.
    ///
    /// # Parameters
    ///
    /// * `number_model` - Integer or feature-selected binary64 range contract.
    ///
    /// # Returns
    ///
    /// This scanner with the selected number model.
    #[inline(always)]
    pub(crate) const fn with_number_model(mut self, number_model: JsonNumberModel) -> Self {
        self.number_model = number_model;
        self
    }

    /// Selects when lexical tokens are admitted; defaults to
    /// [`JsonLexicalAdmissionPolicy::Immediate`].
    ///
    /// The ordering depends on token kind: `Immediate` checks delimiters before
    /// admission for literals and numbers, while `TokenFirst` validates
    /// complete tokens before following syntax is checked. This policy does
    /// not enable or disable value-resource accounting.
    pub(crate) const fn with_admission_policy(mut self, policy: JsonLexicalAdmissionPolicy) -> Self {
        self.admission_policy = policy;
        self
    }

    /// Sets an optional root-inclusive maximum depth; `None` disables this
    /// additional cap.
    ///
    /// A limit of zero rejects the root value. This check is independent of
    /// `has_value_limits`, which controls staging of ordinary value
    /// measurements.
    pub(crate) const fn with_depth_limit(mut self, depth_limit: Option<Q>) -> Self {
        self.depth_limit = depth_limit;
        self
    }

    /// Sets an optional maximum number of encoded bytes inside each string
    /// token.
    ///
    /// The surrounding quotes are excluded; `None` disables this token-local
    /// check, while zero permits only empty encoded string contents. This
    /// check is independent of `has_value_limits` and does not measure
    /// decoded string bytes.
    pub(crate) const fn with_max_encoded_string_bytes(mut self, maximum: Option<usize>) -> Self {
        self.max_encoded_string_bytes = maximum;
        self
    }

    /// Validates and charges one complete JSON document.
    ///
    /// # Parameters
    ///
    /// * `input` - Byte sequence containing exactly one JSON document.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the entire input is valid and all measurements are staged.
    ///
    /// # Errors
    ///
    /// Returns [`JsonLexicalError::Budget`] for the first resource violation,
    /// or [`JsonLexicalError::Syntax`] when `input` is not one complete JSON
    /// value. All value measurements remain staged in the caller's transaction.
    #[inline]
    pub(crate) fn scan(&mut self, input: &[u8]) -> Result<(), JsonLexicalError<R, Q>> {
        self.scan_with_events(input, |_| {})
    }

    /// Validates a document and synchronously emits each admitted token.
    ///
    /// Events may precede a later document error; consumers must only
    /// materialize input after a complete admission pass has succeeded.
    ///
    /// # Type Parameters
    ///
    /// * `F` - Callback type used to receive lexical events.
    ///
    /// # Parameters
    ///
    /// * `input` - Byte sequence containing exactly one JSON document.
    /// * `emit` - Synchronous callback invoked for each admitted token.
    ///
    /// # Returns
    ///
    /// `Ok(())` after complete validation and event emission.
    ///
    /// # Errors
    ///
    /// Returns a budget error for the first resource violation or a syntax
    /// error when the input is not one complete JSON value.
    pub(crate) fn scan_with_events<'input, F>(
        &mut self,
        input: &'input [u8],
        mut emit: F,
    ) -> Result<(), JsonLexicalError<R, Q>>
    where
        F: FnMut(JsonLexicalEvent<'input>),
    {
        let mut cursor = JsonLexicalCursor::new(input, &mut *self.transaction, self.has_value_limits);
        cursor.set_number_model(self.number_model);
        cursor.set_admission_policy(self.admission_policy);
        cursor.set_depth_limit(self.depth_limit);
        cursor.set_max_encoded_string_bytes(self.max_encoded_string_bytes);
        let mut adapter = |point| match point {
            JsonLexicalScanPoint::Event(event) => {
                emit(event);
                true
            }
            _ => true,
        };
        let mut stack: Vec<JsonLexicalContainerFrame> = Vec::new();
        cursor.skip_whitespace();
        cursor.set_context(JsonDecodeContext::Value);
        cursor.value(self.root_depth, &mut stack, &mut adapter)?;
        while let Some(frame) = stack.pop() {
            cursor.resume(frame, &mut stack, &mut adapter)?;
        }
        cursor.skip_whitespace();
        cursor.set_context(JsonDecodeContext::TrailingInput);
        if cursor.is_at_end() {
            Ok(())
        } else {
            Err(cursor.syntax(JsonLexicalErrorReason::TrailingCharacters))
        }
    }

    /// Scans through the shared grammar and stops at the first reported
    /// failure.
    ///
    /// A consumer error is returned as [`JsonLexicalScanError::Consumer`];
    /// scanner syntax or budget failures are returned as `Lexical`, except
    /// an encoded-string byte-limit violation, which is returned as
    /// `EncodedStringLimit`. A failed consumer callback stops further
    /// callbacks and scanning immediately. The number delimiter is checked
    /// after `before_number` and its event, so that callback does not
    /// establish that the following delimiter or document is valid.
    pub(crate) fn scan_with_consumer<'input, C>(
        &mut self,
        input: &'input [u8],
        consumer: &mut C,
    ) -> Result<(), JsonLexicalScanError<C::Error, R, Q>>
    where
        C: JsonLexicalConsumer<'input>,
    {
        let mut cursor = JsonLexicalCursor::new(input, &mut *self.transaction, self.has_value_limits);
        cursor.set_number_model(self.number_model);
        cursor.set_admission_policy(self.admission_policy);
        cursor.set_depth_limit(self.depth_limit);
        cursor.set_max_encoded_string_bytes(self.max_encoded_string_bytes);
        cursor.set_defer_number_delimiter(true);
        let mut consumer_error = None;
        let mut dispatch = |point| {
            let result = match point {
                JsonLexicalScanPoint::BeforeValue { depth, offset } => consumer.before_value(depth, offset),
                JsonLexicalScanPoint::BeforeKey { offset } => consumer.before_key(offset),
                JsonLexicalScanPoint::BeforeNumber { token, offset } => consumer.before_number(token, offset),
                JsonLexicalScanPoint::Event(event) => consumer.event(event),
            };
            match result {
                Ok(()) => true,
                Err(error) => {
                    consumer_error = Some(error);
                    false
                }
            }
        };
        let mut stack = Vec::new();
        let scan = (|| {
            cursor.skip_whitespace();
            cursor.set_context(JsonDecodeContext::Value);
            cursor.value(self.root_depth, &mut stack, &mut dispatch)?;
            while let Some(frame) = stack.pop() {
                cursor.resume(frame, &mut stack, &mut dispatch)?;
            }
            cursor.skip_whitespace();
            cursor.set_context(JsonDecodeContext::TrailingInput);
            if cursor.is_at_end() {
                Ok(())
            } else {
                Err(cursor.syntax(JsonLexicalErrorReason::TrailingCharacters))
            }
        })();
        match scan {
            Ok(()) => Ok(()),
            Err(JsonLexicalError::Syntax(failure)) => match failure.reason {
                JsonLexicalErrorReason::EncodedStringLimit { maximum, observed } => {
                    Err(JsonLexicalScanError::EncodedStringLimit {
                        maximum,
                        observed,
                        offset: failure.offset,
                    })
                }
                JsonLexicalErrorReason::ConsumerStopped if consumer_error.is_some() => Err(
                    JsonLexicalScanError::Consumer(consumer_error.expect("consumer failure was captured")),
                ),
                _ if consumer_error.is_some() => Err(JsonLexicalScanError::Consumer(consumer_error.unwrap())),
                _ => Err(JsonLexicalScanError::Lexical(JsonLexicalError::Syntax(failure))),
            },
            Err(_error) if consumer_error.is_some() => Err(JsonLexicalScanError::Consumer(consumer_error.unwrap())),
            Err(error) => Err(JsonLexicalScanError::Lexical(error)),
        }
    }
}

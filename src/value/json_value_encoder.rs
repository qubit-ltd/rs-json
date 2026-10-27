// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Strict projection from Serde values into materialized JSON values.

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_budget::json::JsonValueTransaction;
use serde::Serialize;
use serde_json::Value;

use self::internal::JsonValueEncodeContext;
use self::internal::JsonValueEncodeFailure;
use self::internal::JsonValueSerializer;
use crate::encode::JsonSerializationError;
use crate::value::JsonValueEncodeError;
use crate::value::SafeJsonValue;

mod internal;

/// Projects serializable values into strict materialized JSON.
///
/// The encoder accepts the full signed and unsigned 64-bit JSON integer range,
/// rejects wider numeric values and non-finite floats, validates map keys, and
/// rejects duplicate object keys. It owns no resource budget; use
/// [`crate::encode::JsonEncoder`] when encoded text and resource accounting are
/// required. The transaction-aware method stages native JSON measurements and
/// protects its returned tree until the caller explicitly unwraps it.
///
/// # Examples
///
/// ```
/// use qubit_json::value::JsonValueEncoder;
/// use serde_json::json;
///
/// let encoder = JsonValueEncoder::new();
/// let value = encoder.encode(&json!({"ok": true}))?;
/// assert_eq!(value, json!({"ok": true}));
/// # Ok::<(), qubit_json::encode::JsonSerializationError>(())
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonValueEncoder {
    /// Prevents external struct literals while reserving room for future
    /// policy.
    private: (),
}

impl JsonValueEncoder {
    /// Creates a strict encoder with the default immutable policy.
    ///
    /// # Returns
    ///
    /// A reusable encoder that performs no resource accounting.
    #[must_use]
    #[inline(always)]
    pub const fn new() -> Self {
        Self { private: () }
    }

    /// Projects one serializable value into strict JSON.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Source type traversed through its Serde representation.
    ///
    /// # Parameters
    ///
    /// * `value` - Borrowed value to project without taking ownership.
    ///
    /// # Returns
    ///
    /// The fully materialized JSON value.
    ///
    /// # Errors
    ///
    /// Returns a strict value error whose precise kind distinguishes number,
    /// object-key, RawValue, capacity, serializer-contract, and opaque custom
    /// failures without retaining arbitrary diagnostic text.
    pub fn encode<T>(&self, value: &T) -> Result<Value, JsonSerializationError>
    where
        T: Serialize + ?Sized,
    {
        let () = self.private;
        let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
        let mut transaction = budget.transaction();
        let owner = match self.encode_in_transaction(value, &mut transaction) {
            Ok(owner) => owner,
            Err(JsonValueEncodeError::Serialization(error)) => return Err(error),
            Err(JsonValueEncodeError::Budget(_)) => {
                return Err(JsonSerializationError::new(
                    crate::encode::JsonSerializationErrorKind::CustomSerialization,
                ));
            }
            Err(JsonValueEncodeError::PrivateTextLimitExceeded { .. }) => {
                return Err(JsonSerializationError::new(
                    crate::encode::JsonSerializationErrorKind::CustomSerialization,
                ));
            }
        };
        transaction
            .commit()
            .map_err(|_| JsonSerializationError::new(crate::encode::JsonSerializationErrorKind::CustomSerialization))?;
        Ok(owner.into_value())
    }

    /// Projects one value while staging native measurements in a caller-owned
    /// transaction.
    ///
    /// The transaction remains borrowed and uncommitted. Repeated successful
    /// calls may be committed together by the caller. If this method returns an
    /// error or unwinds, the caller must drop the entire transaction rather
    /// than committing its staged state.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Serde source type; it may be dynamically sized.
    /// * `R` - Resource identity retained in measured budget failures.
    /// * `Q` - Exact unsigned quantity type used by the budget.
    ///
    /// # Parameters
    ///
    /// * `value` - Borrowed value to project.
    /// * `transaction` - Caller-owned value transaction receiving native
    ///   measurements.
    ///
    /// # Returns
    ///
    /// A protected JSON owner when serialization and all measurements succeed.
    ///
    /// # Errors
    ///
    /// Returns the first measured budget error or serialization failure.
    pub fn encode_in_transaction<T, R, Q>(
        &self,
        value: &T,
        transaction: &mut JsonValueTransaction<'_, R, Q>,
    ) -> Result<SafeJsonValue, JsonValueEncodeError<R, Q>>
    where
        T: Serialize + ?Sized,
        R: Clone,
        Q: ResourceQuantity,
    {
        self.encode_in_transaction_impl(value, transaction, None)
    }

    /// Projects one value while bounding each private RawValue or Number text
    /// payload before it is copied or appended.
    ///
    /// The limit counts UTF-8 bytes in each payload independently and includes
    /// the exact boundary. It does not limit ordinary strings or consume the
    /// transaction's native value budget. If this method fails, drop the whole
    /// transaction rather than committing staged state.
    ///
    /// # Parameters
    ///
    /// * `value` - Borrowed Serde value to project.
    /// * `transaction` - Caller-owned transaction receiving native value
    ///   measurements.
    /// * `max_private_text_bytes` - Inclusive UTF-8 byte limit for each private
    ///   RawValue or Number payload.
    ///
    /// # Returns
    ///
    /// A protected JSON owner when serialization and measurement succeed.
    ///
    /// # Errors
    ///
    /// Returns the first budget or serialization failure, or
    /// `PrivateTextLimitExceeded` when a private text payload exceeds the
    /// configured byte limit. The limit error does not contain payload text.
    pub fn encode_in_transaction_with_private_text_limit<T, R, Q>(
        &self,
        value: &T,
        transaction: &mut JsonValueTransaction<'_, R, Q>,
        max_private_text_bytes: usize,
    ) -> Result<SafeJsonValue, JsonValueEncodeError<R, Q>>
    where
        T: Serialize + ?Sized,
        R: Clone,
        Q: ResourceQuantity,
    {
        self.encode_in_transaction_impl(value, transaction, Some(max_private_text_bytes))
    }

    fn encode_in_transaction_impl<T, R, Q>(
        &self,
        value: &T,
        transaction: &mut JsonValueTransaction<'_, R, Q>,
        max_private_text_bytes: Option<usize>,
    ) -> Result<SafeJsonValue, JsonValueEncodeError<R, Q>>
    where
        T: Serialize + ?Sized,
        R: Clone,
        Q: ResourceQuantity,
    {
        let context = std::cell::RefCell::new(JsonValueEncodeContext::new(transaction, max_private_text_bytes));
        let result = value.serialize(JsonValueSerializer::new(&context, 1));
        if let Err(error) = result.as_ref() {
            let _ = context.borrow_mut().record_serialization_error(*error);
        }
        match context.into_inner().take_failure() {
            Some(JsonValueEncodeFailure::Budget(error)) => Err(JsonValueEncodeError::Budget(error)),
            Some(JsonValueEncodeFailure::Serialization(error)) => Err(JsonValueEncodeError::Serialization(error)),
            Some(JsonValueEncodeFailure::PrivateTextLimitExceeded { maximum }) => {
                Err(JsonValueEncodeError::PrivateTextLimitExceeded { maximum })
            }
            None => result.map_err(JsonValueEncodeError::Serialization),
        }
    }
}

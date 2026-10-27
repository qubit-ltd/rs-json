// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared transaction and first-failure state for one value projection.

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonContainerKind;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonValueTransaction;

use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;

pub(in crate::value::json_value_encoder) enum JsonValueEncodeFailure<R, Q>
where
    Q: ResourceQuantity,
{
    Budget(MeasuredBudgetError<R, Q>),
    Serialization(JsonSerializationError),
    PrivateTextLimitExceeded { maximum: usize },
}

/// Carries one projection's staged budget state and first failure.
///
/// A recorded error is sticky: later checks return its serialization form and
/// cannot replace the original failure. Private text limits are configured by
/// the owning encoder and apply to each private payload independently.
pub(in crate::value::json_value_encoder) struct JsonValueEncodeContext<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    failure: Option<JsonValueEncodeFailure<R, Q>>,
    max_private_text_bytes: Option<usize>,
}

impl<'transaction, 'budget, R, Q> JsonValueEncodeContext<'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a context using the owning encoder's per-payload text limit.
    ///
    /// `None` preserves the legacy unbounded private-text behavior.
    pub(in crate::value::json_value_encoder) fn new(
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
        max_private_text_bytes: Option<usize>,
    ) -> Self {
        Self {
            transaction,
            failure: None,
            max_private_text_bytes,
        }
    }

    pub(in crate::value::json_value_encoder) fn transaction(&mut self) -> &mut JsonValueTransaction<'budget, R, Q> {
        self.transaction
    }

    pub(in crate::value::json_value_encoder) fn has_limits(&self) -> bool {
        self.transaction.has_limits()
    }

    pub(in crate::value::json_value_encoder) fn has_failure(&self) -> bool {
        self.failure.is_some()
    }

    /// Checks one private payload's UTF-8 byte length without charging budget.
    ///
    /// An over-limit result becomes the retained first failure.
    pub(in crate::value::json_value_encoder) fn check_private_text_bytes(
        &mut self,
        length: usize,
    ) -> Result<(), JsonSerializationError> {
        if self.failure.is_some() {
            return Err(self.ser_error());
        }
        if let Some(maximum) = self.max_private_text_bytes
            && length > maximum
        {
            self.failure = Some(JsonValueEncodeFailure::PrivateTextLimitExceeded { maximum });
            return Err(self.ser_error());
        }
        Ok(())
    }

    /// Charges a measurement to the staged transaction.
    ///
    /// On rejection, retains the first budget failure. This is distinct from
    /// `check_admit`, which verifies a prospective measurement without charging
    /// it.
    pub(in crate::value::json_value_encoder) fn admit(
        &mut self,
        measurement: JsonMeasurement,
    ) -> Result<(), JsonSerializationError> {
        if self.failure.is_some() {
            return Err(self.ser_error());
        }
        if !self.transaction.has_limits() {
            return Ok(());
        }
        if let Err(error) = self.transaction.try_admit(measurement) {
            self.failure = Some(JsonValueEncodeFailure::Budget(error));
            return Err(self.ser_error());
        }
        Ok(())
    }

    /// Checks a prospective measurement without changing the transaction.
    ///
    /// A rejected check is still retained as the first projection failure, so
    /// later operations cannot proceed as if the preflight had succeeded.
    pub(in crate::value::json_value_encoder) fn check_admit(
        &mut self,
        measurement: JsonMeasurement,
    ) -> Result<(), JsonSerializationError> {
        if self.failure.is_some() {
            return Err(self.ser_error());
        }
        if !self.transaction.has_limits() {
            return Ok(());
        }
        if let Err(error) = self.transaction.check_admit(measurement) {
            self.failure = Some(JsonValueEncodeFailure::Budget(error));
            return Err(self.ser_error());
        }
        Ok(())
    }

    /// Enters and charges one container at its root-inclusive depth.
    ///
    /// A rejected entry is retained as the first budget failure.
    pub(in crate::value::json_value_encoder) fn enter_container(
        &mut self,
        kind: JsonContainerKind,
        depth: usize,
    ) -> Result<(), JsonSerializationError> {
        if self.failure.is_some() {
            return Err(self.ser_error());
        }
        if !self.transaction.has_limits() {
            return Ok(());
        }
        if let Err(error) = self.transaction.try_enter_container(kind, depth) {
            self.failure = Some(JsonValueEncodeFailure::Budget(error));
            return Err(self.ser_error());
        }
        Ok(())
    }

    /// Checks a container's prospective item count without charging it.
    ///
    /// A rejected count check is retained as the first budget failure.
    pub(in crate::value::json_value_encoder) fn check_container_count(
        &mut self,
        kind: JsonContainerKind,
        count: usize,
    ) -> Result<(), JsonSerializationError> {
        if self.failure.is_some() {
            return Err(self.ser_error());
        }
        if !self.transaction.has_limits() {
            return Ok(());
        }
        if let Err(error) = self.transaction.check_container_count(kind, count) {
            self.failure = Some(JsonValueEncodeFailure::Budget(error));
            return Err(self.ser_error());
        }
        Ok(())
    }

    /// Retains a serialization error only when no earlier failure exists.
    pub(in crate::value::json_value_encoder) fn record_serialization_error(
        &mut self,
        error: JsonSerializationError,
    ) -> JsonSerializationError {
        if self.failure.is_none() {
            self.failure = Some(JsonValueEncodeFailure::Serialization(error));
        }
        self.ser_error()
    }

    /// Retains a measured budget failure only when no earlier failure exists.
    pub(in crate::value::json_value_encoder) fn record_budget_failure(
        &mut self,
        error: MeasuredBudgetError<R, Q>,
    ) -> JsonSerializationError {
        if self.failure.is_none() {
            self.failure = Some(JsonValueEncodeFailure::Budget(error));
        }
        self.ser_error()
    }

    pub(in crate::value::json_value_encoder) fn ser_error(&self) -> JsonSerializationError {
        match self.failure.as_ref() {
            Some(JsonValueEncodeFailure::Serialization(error)) => *error,
            Some(JsonValueEncodeFailure::Budget(_)) => {
                JsonSerializationError::new(JsonSerializationErrorKind::CustomSerialization)
            }
            Some(JsonValueEncodeFailure::PrivateTextLimitExceeded { .. }) => {
                JsonSerializationError::new(JsonSerializationErrorKind::CustomSerialization)
            }
            None => JsonSerializationError::new(JsonSerializationErrorKind::CustomSerialization),
        }
    }

    /// Consumes the context and returns its retained first failure, if any.
    pub(in crate::value::json_value_encoder) fn take_failure(self) -> Option<JsonValueEncodeFailure<R, Q>> {
        self.failure
    }
}

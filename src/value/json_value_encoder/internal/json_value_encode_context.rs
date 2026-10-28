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

/// The first failure retained while projecting one JSON value.
///
/// It records a measured budget rejection, a structured serialization error,
/// or a private payload exceeding its per-payload UTF-8 byte limit. Serde may
/// receive an opaque error to stop traversal; the owning encoder recovers the
/// retained failure and its original category afterward.
pub(in crate::value::json_value_encoder) enum JsonValueEncodeFailure<R, Q>
where
    Q: ResourceQuantity,
{
    /// A measured budget rejected a value, container, or collection count.
    Budget(MeasuredBudgetError<R, Q>),
    /// Serde or the projection serializer reported a structured error.
    Serialization(JsonSerializationError),
    /// A private payload exceeded the inclusive UTF-8 byte limit in `maximum`.
    ///
    /// `maximum` is the per-payload UTF-8 byte threshold.
    PrivateTextLimitExceeded { maximum: usize },
}

/// Carries one projection's staged budget state and first failure.
///
/// A recorded error is sticky: later checks return its serialization form and
/// cannot replace the original failure. Private text limits are configured by
/// the owning encoder and apply to each private payload independently. Budget
/// charges remain staged in `transaction` until its owner commits it.
pub(in crate::value::json_value_encoder) struct JsonValueEncodeContext<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Mutable staged transaction shared by the projection.
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    /// First failure observed by this projection, retained until consumed.
    failure: Option<JsonValueEncodeFailure<R, Q>>,
    /// Optional inclusive UTF-8 byte limit applied separately to each payload.
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

    /// Borrows the staged transaction for inspection or a budget operation.
    ///
    /// This does not commit the transaction; commit remains the responsibility
    /// of the owning encoder.
    pub(in crate::value::json_value_encoder) fn transaction(&mut self) -> &mut JsonValueTransaction<'budget, R, Q> {
        self.transaction
    }

    /// Reports whether the staged budget transaction has resource limits.
    ///
    /// This says nothing about the independent private text limit or whether a
    /// projection failure has already been recorded.
    pub(in crate::value::json_value_encoder) fn has_limits(&self) -> bool {
        self.transaction.has_limits()
    }

    /// Reports whether this projection has retained a first failure.
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

    /// Produces the error form used to stop a Serde traversal.
    ///
    /// A structured serialization failure keeps its category. Budget and
    /// private text limit failures are represented by an opaque custom error;
    /// the owning encoder recovers their actual categories from
    /// [`Self::take_failure`]. With no retained failure, this also returns
    /// a generic custom error.
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
    ///
    /// This is the recovery path for the original failure category after a
    /// Serde traversal was interrupted by [`Self::ser_error`].
    pub(in crate::value::json_value_encoder) fn take_failure(self) -> Option<JsonValueEncodeFailure<R, Q>> {
        self.failure
    }
}

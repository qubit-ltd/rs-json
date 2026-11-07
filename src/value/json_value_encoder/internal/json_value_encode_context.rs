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

use super::JsonValueEncodeFailure;
use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;

/// Carries one projection's staged budget state and first failure.
///
/// A recorded error is sticky: later checks return its serialization form and
/// cannot replace the original failure. Private text limits are configured by
/// the owning encoder and apply to each private payload independently. Budget
/// charges remain staged in `transaction` until its owner commits it.
///
/// # Type Parameters
///
/// - `'transaction`: Lifetime of the mutable staged transaction borrow.
/// - `'budget`: Lifetime of budget data borrowed by the transaction.
/// - `R`: Resource type tracked by the transaction.
/// - `Q`: Quantity type used by the transaction; it must implement
///   [`ResourceQuantity`].
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
    ///
    /// # Parameters
    ///
    /// - `transaction`: Mutable staged transaction owned and eventually
    ///   committed by the caller.
    /// - `max_private_text_bytes`: Optional inclusive UTF-8 byte limit for each
    ///   private payload.
    ///
    /// # Returns
    ///
    /// A context that borrows the staged transaction and retains no failure.
    #[must_use]
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
    ///
    /// # Returns
    ///
    /// A mutable borrow of the staged transaction.
    #[must_use = "the staged transaction borrow must be used"]
    pub(in crate::value::json_value_encoder) fn transaction(
        &mut self,
    ) -> &mut JsonValueTransaction<'budget, R, Q> {
        self.transaction
    }

    /// Reports whether the staged budget transaction has resource limits.
    ///
    /// This says nothing about the independent private text limit or whether a
    /// projection failure has already been recorded.
    ///
    /// # Returns
    ///
    /// `true` when the transaction enforces resource limits.
    #[must_use]
    pub(in crate::value::json_value_encoder) fn has_limits(&self) -> bool {
        self.transaction.has_limits()
    }

    /// Reports whether this projection has retained a first failure.
    ///
    /// # Returns
    ///
    /// `true` when a failure is already retained.
    #[must_use]
    pub(in crate::value::json_value_encoder) fn has_failure(&self) -> bool {
        self.failure.is_some()
    }

    /// Checks one private payload's UTF-8 byte length without charging budget.
    ///
    /// An over-limit result becomes the retained first failure.
    ///
    /// # Parameters
    ///
    /// - `length`: UTF-8 byte length of the private payload.
    ///
    /// # Errors
    ///
    /// Returns the traversal error when a previous failure exists or the
    /// configured per-payload limit is exceeded.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the payload is within the configured limit; otherwise,
    /// the traversal error.
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
    ///
    /// # Parameters
    ///
    /// - `measurement`: Resource usage to charge to the staged transaction.
    ///
    /// # Errors
    ///
    /// Returns the traversal error if a prior failure exists or the budget
    /// rejects the measurement.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the measurement is charged; otherwise, the traversal
    /// error.
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
    ///
    /// # Parameters
    ///
    /// - `measurement`: Prospective resource usage to check without charging.
    ///
    /// # Errors
    ///
    /// Returns the traversal error if a prior failure exists or the budget
    /// rejects the prospective measurement.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the prospective measurement is allowed; otherwise, the
    /// traversal error.
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
    ///
    /// # Parameters
    ///
    /// - `kind`: Container category used by budget accounting.
    /// - `depth`: Container depth, including the root as depth one.
    ///
    /// # Errors
    ///
    /// Returns the traversal error if a prior failure exists or budget
    /// accounting rejects the container.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the container is entered; otherwise, the traversal error.
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
    ///
    /// # Parameters
    ///
    /// - `kind`: Container category used by budget accounting.
    /// - `count`: Prospective number of items in the container.
    ///
    /// # Errors
    ///
    /// Returns the traversal error if a prior failure exists or budget
    /// accounting rejects the count.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the item count is allowed; otherwise, the traversal error.
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
    ///
    /// # Parameters
    ///
    /// - `error`: Serialization error to retain if this is the first failure.
    ///
    /// # Returns
    ///
    /// Returns the traversal error corresponding to the retained first failure.
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
    ///
    /// # Parameters
    ///
    /// - `error`: Measured budget error to retain if this is the first failure.
    ///
    /// # Returns
    ///
    /// Returns the traversal error corresponding to the retained first failure.
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
    ///
    /// # Returns
    ///
    /// The serialization error used to stop or report the Serde traversal.
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
    ///
    /// # Returns
    ///
    /// The first retained failure, or `None` if the projection completed
    /// without recording one.
    pub(in crate::value::json_value_encoder) fn take_failure(
        self,
    ) -> Option<JsonValueEncodeFailure<R, Q>> {
        self.failure
    }
}

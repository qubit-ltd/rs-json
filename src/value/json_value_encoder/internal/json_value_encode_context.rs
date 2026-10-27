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
}

pub(in crate::value::json_value_encoder) struct JsonValueEncodeContext<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    failure: Option<JsonValueEncodeFailure<R, Q>>,
}

impl<'transaction, 'budget, R, Q> JsonValueEncodeContext<'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    pub(in crate::value::json_value_encoder) fn new(
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    ) -> Self {
        Self {
            transaction,
            failure: None,
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

    pub(in crate::value::json_value_encoder) fn record_serialization_error(
        &mut self,
        error: JsonSerializationError,
    ) -> JsonSerializationError {
        if self.failure.is_none() {
            self.failure = Some(JsonValueEncodeFailure::Serialization(error));
        }
        self.ser_error()
    }

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
            None => JsonSerializationError::new(JsonSerializationErrorKind::CustomSerialization),
        }
    }

    pub(in crate::value::json_value_encoder) fn take_failure(self) -> Option<JsonValueEncodeFailure<R, Q>> {
        self.failure
    }
}

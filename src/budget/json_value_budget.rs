// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tracks mutable accounting for one JSON value traversal.

use qubit_budget::BudgetError;
use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceBudget;
use qubit_budget::ResourceLimit;
use qubit_budget::ResourceQuantity;
use qubit_budget::StructureBudget;

use super::JsonResource;
use super::JsonValueLimits;

/// Mutable accounting for JSON nodes, structure and value payloads.
///
/// Structural checks and node accounting are delegated to [`StructureBudget`].
/// Object-key, string and number payloads share one optional cumulative budget.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub struct JsonValueBudget<R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    /// Immutable limits used by this value traversal.
    limits: JsonValueLimits<R, Q>,

    /// Shared structural accounting for this value traversal.
    structure: StructureBudget<R, Q>,

    /// Optional cumulative payload accounting for keys, strings and numbers.
    payload: Option<ResourceBudget<R, Q>>,
}

impl<R, Q> JsonValueBudget<R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a fresh JSON value budget from one immutable limit
    /// configuration.
    #[inline]
    pub fn new(limits: JsonValueLimits<R, Q>) -> Self {
        let structure = limits.structure_limits().budget();
        let payload = limits
            .payload_bytes_limit()
            .cloned()
            .map(ResourceBudget::from_limit);
        Self {
            limits,
            structure,
            payload,
        }
    }

    /// Checks a root-inclusive JSON nesting depth.
    #[inline]
    pub fn check_depth(&self, actual: Q) -> Result<(), BudgetError<R, Q>> {
        self.structure.check_depth(actual)
    }

    /// Charges one JSON node to this traversal's cumulative node budget.
    #[inline]
    pub fn charge_node(&mut self) -> Result<(), BudgetError<R, Q>> {
        self.structure.charge_node()
    }

    /// Charges several JSON nodes atomically to this traversal's node budget.
    #[inline]
    pub fn charge_nodes(&mut self, amount: Q) -> Result<(), BudgetError<R, Q>> {
        self.structure.charge_nodes(amount)
    }

    /// Checks the item count of one JSON array.
    #[inline]
    pub fn check_sequence_items(&self, actual: Q) -> Result<(), BudgetError<R, Q>> {
        self.structure.check_sequence_items(actual)
    }

    /// Checks the entry count of one JSON object.
    #[inline]
    pub fn check_map_entries(&self, actual: Q) -> Result<(), BudgetError<R, Q>> {
        self.structure.check_map_entries(actual)
    }

    /// Checks the byte length of one JSON object key.
    #[inline]
    pub fn check_key_bytes(&self, actual: Q) -> Result<(), BudgetError<R, Q>> {
        self.structure.check_key_bytes(actual)
    }

    /// Checks the byte length of one JSON string value.
    #[inline]
    pub fn check_string_bytes(&self, actual: Q) -> Result<(), BudgetError<R, Q>> {
        self.limits
            .string_bytes_limit()
            .map_or(Ok(()), |limit| limit.check(actual))
    }

    /// Checks the byte length of one JSON number representation.
    #[inline]
    pub fn check_number_bytes(&self, actual: Q) -> Result<(), BudgetError<R, Q>> {
        self.limits
            .number_bytes_limit()
            .map_or(Ok(()), |limit| limit.check(actual))
    }

    /// Checks a value depth and charges its node atomically.
    #[inline]
    pub fn enter_node(&mut self, depth: Q) -> Result<(), BudgetError<R, Q>> {
        self.structure.enter_node(depth)
    }

    /// Checks an array's depth and item count, then charges its node
    /// atomically.
    #[inline]
    pub fn enter_array(&mut self, depth: Q, items: Q) -> Result<(), BudgetError<R, Q>> {
        self.structure.enter_sequence(depth, items)
    }

    /// Checks an object's depth and entry count, then charges its node
    /// atomically.
    #[inline]
    pub fn enter_object(&mut self, depth: Q, entries: Q) -> Result<(), BudgetError<R, Q>> {
        self.structure.enter_map(depth, entries)
    }

    /// Converts and admits one JSON value measured with native `usize` values.
    #[inline]
    pub fn enter_node_usize(&mut self, depth: usize) -> Result<(), MeasuredBudgetError<R, Q>> {
        let depth = self.convert_usize(depth, self.limits.structure_limits().depth_limit())?;
        self.enter_node(depth).map_err(MeasuredBudgetError::from)
    }

    /// Converts and admits one JSON array measured with native `usize` values.
    #[inline]
    pub fn enter_array_usize(
        &mut self,
        depth: usize,
        items: usize,
    ) -> Result<(), MeasuredBudgetError<R, Q>> {
        let depth = self.convert_usize(depth, self.limits.structure_limits().depth_limit())?;
        let items =
            self.convert_usize(items, self.limits.structure_limits().sequence_items_limit())?;
        self.enter_array(depth, items)
            .map_err(MeasuredBudgetError::from)
    }

    /// Converts and admits one JSON object measured with native `usize` values.
    #[inline]
    pub fn enter_object_usize(
        &mut self,
        depth: usize,
        entries: usize,
    ) -> Result<(), MeasuredBudgetError<R, Q>> {
        let depth = self.convert_usize(depth, self.limits.structure_limits().depth_limit())?;
        let entries =
            self.convert_usize(entries, self.limits.structure_limits().map_entries_limit())?;
        self.enter_object(depth, entries)
            .map_err(MeasuredBudgetError::from)
    }

    /// Converts and checks one native array-item count.
    #[inline]
    pub fn check_sequence_items_usize(
        &self,
        actual: usize,
    ) -> Result<(), MeasuredBudgetError<R, Q>> {
        let actual = self.convert_usize(
            actual,
            self.limits.structure_limits().sequence_items_limit(),
        )?;
        self.check_sequence_items(actual)
            .map_err(MeasuredBudgetError::from)
    }

    /// Converts and checks one native object-entry count.
    #[inline]
    pub fn check_map_entries_usize(&self, actual: usize) -> Result<(), MeasuredBudgetError<R, Q>> {
        let actual =
            self.convert_usize(actual, self.limits.structure_limits().map_entries_limit())?;
        self.check_map_entries(actual)
            .map_err(MeasuredBudgetError::from)
    }

    /// Checks and consumes one object key's payload bytes.
    ///
    /// The key point limit is checked before payload accounting, so a rejected
    /// key never changes the cumulative payload budget.
    #[inline]
    pub fn consume_key_bytes(&mut self, amount: Q) -> Result<(), BudgetError<R, Q>> {
        self.check_key_bytes(amount)?;
        self.consume_payload_bytes(amount)
    }

    /// Checks and consumes one string value's payload bytes.
    ///
    /// The string point limit is checked before payload accounting, so a
    /// rejected string never changes the cumulative payload budget.
    #[inline]
    pub fn consume_string_bytes(&mut self, amount: Q) -> Result<(), BudgetError<R, Q>> {
        self.check_string_bytes(amount)?;
        self.consume_payload_bytes(amount)
    }

    /// Checks and consumes one number representation's payload bytes.
    ///
    /// The number point limit is checked before payload accounting, so a
    /// rejected number never changes the cumulative payload budget.
    #[inline]
    pub fn consume_number_bytes(&mut self, amount: Q) -> Result<(), BudgetError<R, Q>> {
        self.check_number_bytes(amount)?;
        self.consume_payload_bytes(amount)
    }

    /// Converts and consumes native object-key bytes.
    #[inline]
    pub fn consume_key_bytes_usize(
        &mut self,
        amount: usize,
    ) -> Result<(), MeasuredBudgetError<R, Q>> {
        let amount =
            self.convert_payload_usize(amount, self.limits.structure_limits().key_bytes_limit())?;
        self.consume_key_bytes(amount)
            .map_err(MeasuredBudgetError::from)
    }

    /// Converts and checks native object-key bytes without consuming payload.
    #[inline]
    pub fn check_key_bytes_usize(&self, amount: usize) -> Result<(), MeasuredBudgetError<R, Q>> {
        let amount =
            self.convert_usize(amount, self.limits.structure_limits().key_bytes_limit())?;
        self.check_key_bytes(amount)
            .map_err(MeasuredBudgetError::from)
    }

    /// Converts and consumes native string bytes.
    #[inline]
    pub fn consume_string_bytes_usize(
        &mut self,
        amount: usize,
    ) -> Result<(), MeasuredBudgetError<R, Q>> {
        let amount = self.convert_payload_usize(amount, self.limits.string_bytes_limit())?;
        self.consume_string_bytes(amount)
            .map_err(MeasuredBudgetError::from)
    }

    /// Converts and checks native string bytes without consuming payload.
    #[inline]
    pub fn check_string_bytes_usize(&self, amount: usize) -> Result<(), MeasuredBudgetError<R, Q>> {
        let amount = self.convert_usize(amount, self.limits.string_bytes_limit())?;
        self.check_string_bytes(amount)
            .map_err(MeasuredBudgetError::from)
    }

    /// Converts and consumes native number bytes.
    #[inline]
    pub fn consume_number_bytes_usize(
        &mut self,
        amount: usize,
    ) -> Result<(), MeasuredBudgetError<R, Q>> {
        let amount = self.convert_payload_usize(amount, self.limits.number_bytes_limit())?;
        self.consume_number_bytes(amount)
            .map_err(MeasuredBudgetError::from)
    }

    /// Converts and checks native number bytes without consuming payload.
    #[inline]
    pub fn check_number_bytes_usize(&self, amount: usize) -> Result<(), MeasuredBudgetError<R, Q>> {
        let amount = self.convert_usize(amount, self.limits.number_bytes_limit())?;
        self.check_number_bytes(amount)
            .map_err(MeasuredBudgetError::from)
    }

    /// Returns the immutable limits used by this traversal.
    #[must_use = "the configured limits determine which JSON values can be accepted"]
    #[inline(always)]
    pub const fn limits(&self) -> &JsonValueLimits<R, Q> {
        &self.limits
    }

    /// Returns the structural accounting state used by this traversal.
    #[must_use = "the structural budget tracks shared JSON value traversal usage"]
    #[inline(always)]
    pub const fn structure_budget(&self) -> &StructureBudget<R, Q> {
        &self.structure
    }

    /// Returns the cumulative payload budget, when payload accounting is
    /// configured.
    #[must_use = "the payload budget tracks consumed JSON key, string and number bytes"]
    #[inline(always)]
    pub const fn payload_budget(&self) -> Option<&ResourceBudget<R, Q>> {
        self.payload.as_ref()
    }

    /// Consumes a payload increment without applying a single-value point
    /// limit.
    ///
    /// This operation is used after a key, string or number point check. A
    /// failed request leaves the optional payload budget unchanged.
    #[inline]
    fn consume_payload_bytes(&mut self, amount: Q) -> Result<(), BudgetError<R, Q>> {
        match &mut self.payload {
            Some(payload) => payload.try_consume(amount),
            None => Ok(()),
        }
    }

    fn convert_usize(
        &self,
        amount: usize,
        limit: Option<&ResourceLimit<R, Q>>,
    ) -> Result<Q, MeasuredBudgetError<R, Q>> {
        let Some(limit) = limit else {
            return Ok(Q::ZERO);
        };
        Q::try_from_usize(amount)
            .map_err(|source| MeasuredBudgetError::quantity(limit.resource().clone(), source))
    }

    fn convert_payload_usize(
        &self,
        amount: usize,
        point_limit: Option<&ResourceLimit<R, Q>>,
    ) -> Result<Q, MeasuredBudgetError<R, Q>> {
        if let Some(limit) = point_limit {
            return Q::try_from_usize(amount)
                .map_err(|source| MeasuredBudgetError::quantity(limit.resource().clone(), source));
        }
        if let Some(payload) = &self.payload {
            return Q::try_from_usize(amount).map_err(|source| {
                MeasuredBudgetError::quantity(payload.resource().clone(), source)
            });
        }
        Ok(Q::ZERO)
    }
}

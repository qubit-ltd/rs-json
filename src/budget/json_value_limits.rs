// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines direction-independent limits for JSON values.

use qubit_budget::ResourceLimit;
use qubit_budget::ResourceQuantity;
use qubit_budget::StructureLimits;

use super::JsonResource;

/// Optional limits for one JSON value traversal.
///
/// `R` identifies resources reported in [`qubit_budget::BudgetError`], while
/// `Q` defines the unsigned quantity used for every measurement. The default
/// uses [`JsonResource`] and [`usize`].
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JsonValueLimits<R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    /// Structural limits shared by JSON value processing.
    structure: StructureLimits<R, Q>,

    /// Optional inclusive maximum byte length of one string value.
    max_string_bytes: Option<ResourceLimit<R, Q>>,

    /// Optional inclusive maximum byte length of one number representation.
    max_number_bytes: Option<ResourceLimit<R, Q>>,

    /// Optional cumulative maximum byte length of JSON value payloads.
    max_payload_bytes: Option<ResourceLimit<R, Q>>,
}

impl<R, Q> Default for JsonValueLimits<R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates a limit set with every value dimension unconfigured.
    fn default() -> Self {
        Self::unconfigured()
    }
}

impl<R, Q> JsonValueLimits<R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates an unconfigured generic value limit set.
    #[inline]
    pub const fn unconfigured() -> Self {
        Self {
            structure: StructureLimits::empty(),
            max_string_bytes: None,
            max_number_bytes: None,
            max_payload_bytes: None,
        }
    }

    /// Configures the inclusive byte limit for one string value.
    #[inline]
    pub fn with_string_bytes_limit(mut self, limit: ResourceLimit<R, Q>) -> Self {
        self.max_string_bytes = Some(limit);
        self
    }

    /// Configures the inclusive byte limit for one number representation.
    #[inline]
    pub fn with_number_bytes_limit(mut self, limit: ResourceLimit<R, Q>) -> Self {
        self.max_number_bytes = Some(limit);
        self
    }

    /// Configures the cumulative byte budget for JSON keys, strings and
    /// numbers.
    #[inline]
    pub fn with_payload_bytes_limit(mut self, limit: ResourceLimit<R, Q>) -> Self {
        self.max_payload_bytes = Some(limit);
        self
    }

    /// Replaces the structural limits used while processing JSON values.
    #[inline]
    pub fn with_structure_limits<S>(mut self, limits: S) -> Self
    where
        S: Into<StructureLimits<R, Q>>,
    {
        self.structure = limits.into();
        self
    }

    /// Returns the structural limits used by this value configuration.
    #[must_use = "the structural limits determine JSON value traversal checks"]
    #[inline]
    pub fn structure_limits(&self) -> StructureLimits<R, Q>
    where
        R: Clone,
    {
        self.structure.clone()
    }

    /// Returns the configured root-inclusive nesting-depth maximum.
    #[must_use]
    #[inline(always)]
    pub const fn max_depth(&self) -> Option<Q> {
        self.structure.max_depth()
    }

    /// Returns the configured cumulative JSON-node maximum.
    #[must_use]
    #[inline(always)]
    pub const fn max_nodes(&self) -> Option<Q> {
        self.structure.max_nodes()
    }

    /// Returns the configured maximum item count for one JSON array.
    #[must_use]
    #[inline(always)]
    pub const fn max_sequence_items(&self) -> Option<Q> {
        self.structure.max_sequence_items()
    }

    /// Returns the configured maximum entry count for one JSON object.
    #[must_use]
    #[inline(always)]
    pub const fn max_map_entries(&self) -> Option<Q> {
        self.structure.max_map_entries()
    }

    /// Returns the configured maximum byte length for one JSON object key.
    #[must_use]
    #[inline(always)]
    pub const fn max_key_bytes(&self) -> Option<Q> {
        self.structure.max_key_bytes()
    }

    /// Returns the complete string-byte limit, when configured.
    #[must_use]
    #[inline(always)]
    pub const fn string_bytes_limit(&self) -> Option<&ResourceLimit<R, Q>> {
        self.max_string_bytes.as_ref()
    }

    /// Returns the complete number-byte limit, when configured.
    #[must_use]
    #[inline(always)]
    pub const fn number_bytes_limit(&self) -> Option<&ResourceLimit<R, Q>> {
        self.max_number_bytes.as_ref()
    }

    /// Returns the complete cumulative payload-byte limit, when configured.
    #[must_use]
    #[inline(always)]
    pub const fn payload_bytes_limit(&self) -> Option<&ResourceLimit<R, Q>> {
        self.max_payload_bytes.as_ref()
    }

    /// Returns the configured maximum byte length for one string value.
    #[must_use]
    #[inline(always)]
    pub const fn max_string_bytes(&self) -> Option<Q> {
        limit_maximum(self.max_string_bytes.as_ref())
    }

    /// Returns the configured maximum byte length for one number
    /// representation.
    #[must_use]
    #[inline(always)]
    pub const fn max_number_bytes(&self) -> Option<Q> {
        limit_maximum(self.max_number_bytes.as_ref())
    }

    /// Returns the configured cumulative payload-byte maximum.
    #[must_use]
    #[inline(always)]
    pub const fn max_payload_bytes(&self) -> Option<Q> {
        limit_maximum(self.max_payload_bytes.as_ref())
    }
}

impl JsonValueLimits<JsonResource, usize> {
    /// Creates a value limit set with every JSON value dimension unconfigured.
    #[inline]
    pub const fn empty() -> Self {
        Self::unconfigured()
    }
}

/// Returns an optional limit maximum without exposing its resource identity.
#[inline]
const fn limit_maximum<R, Q>(limit: Option<&ResourceLimit<R, Q>>) -> Option<Q>
where
    Q: ResourceQuantity,
{
    match limit {
        Some(limit) => Some(limit.maximum()),
        None => None,
    }
}

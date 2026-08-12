// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines JSON encoding limits.
// qubit-style: allow source-test-pair

use qubit_budget::ResourceLimit;
use qubit_budget::ResourceQuantity;

use super::JsonResource;
use super::JsonValueLimits;

/// Optional limits for one JSON encoding session.
///
/// Encoding has a directional output-byte budget and shares all value limits
/// through [`JsonValueLimits`].
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JsonEncodeLimits<R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    /// Optional cumulative byte budget for encoded output.
    output: Option<ResourceLimit<R, Q>>,

    /// Direction-independent JSON value limits.
    value: JsonValueLimits<R, Q>,
}

impl<R, Q> Default for JsonEncodeLimits<R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates an encoding limit set with every dimension unconfigured.
    fn default() -> Self {
        Self::unconfigured()
    }
}

impl<R, Q> JsonEncodeLimits<R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates an unconfigured generic encoding limit set.
    #[inline]
    pub const fn unconfigured() -> Self {
        Self {
            output: None,
            value: JsonValueLimits::unconfigured(),
        }
    }

    /// Configures the cumulative output-byte budget for one encode session.
    #[inline]
    pub fn with_output_bytes_limit(
        mut self,
        limit: ResourceLimit<R, Q>,
    ) -> Self {
        self.output = Some(limit);
        self
    }

    /// Replaces the direction-independent value limits for encoding.
    #[inline]
    pub fn with_value_limits(mut self, limits: JsonValueLimits<R, Q>) -> Self {
        self.value = limits;
        self
    }

    /// Returns the complete output-byte limit, when configured.
    #[must_use]
    #[inline(always)]
    pub const fn output_bytes_limit(&self) -> Option<&ResourceLimit<R, Q>> {
        self.output.as_ref()
    }

    /// Returns the direction-independent value limits used for encoding.
    #[must_use = "the value limits determine which encoded JSON values can be accepted"]
    #[inline]
    pub fn value_limits(&self) -> JsonValueLimits<R, Q>
    where
        R: Clone,
    {
        self.value.clone()
    }

    /// Returns the configured cumulative output-byte maximum.
    #[must_use]
    #[inline(always)]
    pub const fn max_output_bytes(&self) -> Option<Q> {
        match self.output.as_ref() {
            Some(limit) => Some(limit.maximum()),
            None => None,
        }
    }
}

impl JsonEncodeLimits<JsonResource, usize> {
    /// Creates an encoding limit set with every dimension unconfigured.
    #[inline]
    pub const fn empty() -> Self {
        Self::unconfigured()
    }
}

// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines JSON decoding limits.
// qubit-style: allow source-test-pair

use qubit_budget::ResourceLimit;
use qubit_budget::ResourceQuantity;

use super::JsonResource;
use super::JsonValueLimits;

/// Optional limits for one JSON decoding session.
///
/// Decoding has a directional input-byte budget and shares all value limits
/// through [`JsonValueLimits`].
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JsonDecodeLimits<R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    /// Optional cumulative byte budget for decoded input.
    input: Option<ResourceLimit<R, Q>>,

    /// Direction-independent JSON value limits.
    value: JsonValueLimits<R, Q>,
}

impl<R, Q> Default for JsonDecodeLimits<R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates a decoding limit set with every dimension unconfigured.
    fn default() -> Self {
        Self::unconfigured()
    }
}

impl<R, Q> JsonDecodeLimits<R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates an unconfigured generic decoding limit set.
    #[inline]
    pub const fn unconfigured() -> Self {
        Self {
            input: None,
            value: JsonValueLimits::unconfigured(),
        }
    }

    /// Configures the cumulative input-byte budget for one decode session.
    #[inline]
    pub fn with_input_bytes_limit(mut self, limit: ResourceLimit<R, Q>) -> Self {
        self.input = Some(limit);
        self
    }

    /// Replaces the direction-independent value limits for decoding.
    #[inline]
    pub fn with_value_limits(mut self, limits: JsonValueLimits<R, Q>) -> Self {
        self.value = limits;
        self
    }

    /// Returns the complete input-byte limit, when configured.
    #[must_use]
    #[inline(always)]
    pub const fn input_bytes_limit(&self) -> Option<&ResourceLimit<R, Q>> {
        self.input.as_ref()
    }

    /// Returns the direction-independent value limits used for decoding.
    #[must_use = "the value limits determine which decoded JSON values can be accepted"]
    #[inline]
    pub fn value_limits(&self) -> JsonValueLimits<R, Q>
    where
        R: Clone,
    {
        self.value.clone()
    }

    /// Returns the configured cumulative input-byte maximum.
    #[must_use]
    #[inline(always)]
    pub const fn max_input_bytes(&self) -> Option<Q> {
        match self.input.as_ref() {
            Some(limit) => Some(limit.maximum()),
            None => None,
        }
    }
}

impl JsonDecodeLimits<JsonResource, usize> {
    /// Creates a decoding limit set with every dimension unconfigured.
    #[inline]
    pub const fn empty() -> Self {
        Self::unconfigured()
    }
}

// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the typed Serde seed used by strict JSON decoding.

use std::marker::PhantomData;

use serde::Deserialize;
use serde::Deserializer;
use serde::de::DeserializeSeed;

/// Seed adapter that delegates to [`Deserialize`] without allocating state.
pub(in crate::decode) struct TypedSeed<T> {
    /// Type marker carrying the requested deserialization target without
    /// storage.
    marker: PhantomData<fn() -> T>,
}

impl<T> TypedSeed<T> {
    /// Creates a zero-sized seed that delegates to `T::deserialize`.
    #[inline(always)]
    pub(in crate::decode) const fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

impl<'de, T> DeserializeSeed<'de> for TypedSeed<T>
where
    T: Deserialize<'de>,
{
    type Value = T;

    /// Delegates the seed operation to the target type's `Deserialize`
    /// implementation.
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer)
    }
}

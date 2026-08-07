// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the counted failure target used by decoder regression tests.

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use serde::Deserialize;
use serde::Deserializer;
use serde::de;

/// Counts target deserialization attempts made by decoder calls.
static DESERIALIZE_CALLS: AtomicUsize = AtomicUsize::new(0);

/// Target that records one attempt and then rejects otherwise valid JSON.
#[derive(Debug)]
pub(crate) struct CountedFailure;

/// Resets the target deserialization count before one regression assertion.
#[inline(always)]
pub(crate) fn reset_deserialize_calls() {
    DESERIALIZE_CALLS.store(0, Ordering::SeqCst);
}

/// Returns the number of target deserialization attempts recorded so far.
///
/// # Returns
///
/// The current invocation count.
#[inline(always)]
pub(crate) fn deserialize_calls() -> usize {
    DESERIALIZE_CALLS.load(Ordering::SeqCst)
}

impl<'de> Deserialize<'de> for CountedFailure {
    /// Records the invocation before returning an intentional data error.
    ///
    /// # Parameters
    ///
    /// * `deserializer` - Deserializer that supplies one valid JSON string.
    ///
    /// # Returns
    ///
    /// This fixture never returns a value.
    ///
    /// # Errors
    ///
    /// Returns the underlying string error or an intentional data error after
    /// the string has been consumed.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        DESERIALIZE_CALLS.fetch_add(1, Ordering::SeqCst);
        let _ = String::deserialize(deserializer)?;
        Err(de::Error::custom("intentional deserialization failure"))
    }
}

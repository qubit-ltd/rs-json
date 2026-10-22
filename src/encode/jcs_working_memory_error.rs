// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the safe error returned when JCS working memory exceeds its limit.

use std::error::Error;
use std::fmt;

/// Reports when JCS rendering needs more auxiliary memory than the configured
/// limit.
///
/// The error records both byte counts without exposing the rendered value.
///
/// # Examples
///
/// ```
/// use qubit_budget::json::JsonEncodeLimits;
/// use qubit_json::encode::JcsEncoder;
///
/// let limits: JsonEncodeLimits = JsonEncodeLimits::new();
/// let mut encoder = JcsEncoder::with_limits(limits, 0);
/// let error = encoder
///     .to_vec(&serde_json::json!({"key": "value"}))
///     .expect_err("zero working memory cannot render an object");
/// let memory_error = error
///     .working_memory_error()
///     .expect("the failure is caused by the working memory limit");
/// assert_eq!(memory_error.limit(), 0);
/// assert!(memory_error.requested() > memory_error.limit());
/// ```
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JcsWorkingMemoryError {
    /// Maximum auxiliary memory available to the JCS renderer, in bytes.
    limit: usize,
    /// Auxiliary memory required by the rejected rendering request, in bytes.
    requested: usize,
}

impl JcsWorkingMemoryError {
    /// Creates a working memory error with the configured limit and request.
    #[inline]
    pub(crate) const fn new(limit: usize, requested: usize) -> Self {
        Self { limit, requested }
    }

    /// Returns the configured working memory limit in bytes.
    #[must_use]
    #[inline]
    pub const fn limit(&self) -> usize {
        self.limit
    }

    /// Returns the requested working memory in bytes.
    #[must_use]
    #[inline]
    pub const fn requested(&self) -> usize {
        self.requested
    }
}

impl fmt::Display for JcsWorkingMemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "JCS working memory limit exceeded (limit: {}, requested: {})",
            self.limit, self.requested
        )
    }
}

impl Error for JcsWorkingMemoryError {}

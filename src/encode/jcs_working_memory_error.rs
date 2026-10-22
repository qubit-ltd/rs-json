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

/// Reports a JCS working memory request that exceeded the configured limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JcsWorkingMemoryError {
    limit: usize,
    requested: usize,
}

impl JcsWorkingMemoryError {
    /// Creates a working memory error with the configured limit and request.
    #[must_use]
    pub(crate) const fn new(limit: usize, requested: usize) -> Self {
        Self { limit, requested }
    }

    /// Returns the configured working memory limit in bytes.
    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }

    /// Returns the requested working memory in bytes.
    #[must_use]
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

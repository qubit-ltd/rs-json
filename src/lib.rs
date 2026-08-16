// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Resource-aware JSON infrastructure organized into decoding, encoding, and
//! materialized-value domains.
//!
//! * [`decode`] validates strict JSON and normalizes explicitly configured
//!   non-standard text.
//! * [`encode`] serializes values with explicit output and value budgets.
//! * [`value`] constructs and traverses materialized JSON values.
//!
//! `qubit-budget` owns limits, resource identities, and sessions. This crate
//! owns JSON-specific normalization, syntax validation, value construction,
//! and traversal.

#![deny(missing_docs)]

pub mod decode;
pub mod encode;
mod lexical;
pub mod value;

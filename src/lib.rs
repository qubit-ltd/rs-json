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
//!
//! # Number contract
//!
//! Strict text codecs accept negative integers in `i64`, non-negative
//! integers in `u64`, and fractional or exponential tokens representable as
//! finite `f64`. This intentionally includes integers above JavaScript's safe
//! integer limit. Wider exact integers and exact decimals must use strings or
//! an explicit domain wire format. `NumberBytes` limits lexical size; it does
//! not expand the supported numeric range.

#![deny(missing_docs)]

pub mod decode;
pub mod encode;
mod internal;
mod lexical;
pub mod value;

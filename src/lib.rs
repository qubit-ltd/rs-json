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
//! * [`mod@format`] validates and reformats JSON token streams while preserving
//!   their scalar token spellings.
//! * [`value`] constructs and traverses materialized JSON values.
//!
//! `qubit-budget` owns limits, resource identities, and sessions. This crate
//! owns JSON-specific normalization, syntax validation, value construction,
//! and traversal.
//!
//! # Number contract
//!
//! Ordinary strict and normalizing text decoding defaults to the `Integer64`
//! model: negative integers fit `i64`, non-negative integers fit `u64`, and
//! fractional or exponential tokens are finite `f64`. An explicit supported
//! decode entry point can select finite fallback for out-of-range integer
//! tokens; this may round them. `JcsDecoder` instead converts every number to
//! finite binary64. Strict encoding retains its own range checks and does not
//! inherit a decode fallback. Wider exact integers and exact decimals should
//! use strings or an explicit domain wire format. `NumberBytes` limits lexical
//! size; it does not expand the supported numeric range.

#![deny(missing_docs)]

#[cfg(doctest)]
#[doc(hidden)]
mod documentation_examples;

#[cfg(test)]
mod tests;

pub mod decode;
pub mod encode;
pub mod format;
mod internal;
mod lexical;
pub mod value;

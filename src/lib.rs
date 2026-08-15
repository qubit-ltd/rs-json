// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Resource-aware JSON infrastructure organized into four public domains.
//!
//! * [`lenient`] normalizes explicitly configured text noise before direct
//!   Serde deserialization.
//! * [`text`] provides stateful, strict JSON decoders and encoders.
//! * [`value`] constructs [`serde_json::Value`] through a budgeted Serde seed.
//! * [`tree`] iteratively reads or mutates materialized values.
//!
//! `qubit-budget` owns limits, resource identities, and sessions. This crate
//! owns JSON-specific normalization, syntax validation, value construction,
//! and traversal.
//!
//! # Strict decoding
//!
//! ```rust
//! use qubit_budget::json::{JsonDecodeLimits, JsonDecodeSession};
//! use qubit_json::text::JsonTextDecoder;
//!
//! let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
//! let value: serde_json::Value = JsonTextDecoder::new(&mut session)
//!     .decode(br#"{"ok":true}"#)?;
//! assert_eq!(value["ok"], true);
//! # Ok::<(), qubit_json::text::JsonDecodeError<
//! #     qubit_budget::json::JsonResource,
//! # >>(())
//! ```
//!
//! Strict decoding reports [`text::JsonDecodeError`]: budget rejection,
//! syntax rejection, or target deserialization metadata. Strict encoding
//! reports [`text::JsonEncodeError`]: budget rejection, invalid raw JSON,
//! serialization, or writer failure. [`text::JsonSyntaxError`] carries stable
//! syntax location and reason metadata.
//!
//! # Lenient decoding and budgets
//!
//! [`lenient::LenientJsonDecoder`] owns immutable
//! [`lenient::LenientJsonDecodeOptions`]. Its session-aware path retains input
//! charges after an attempt and commits staged value charges only after a
//! complete typed decode. [`lenient::LenientJsonDecodeError`] records the
//! domain-specific normalization, input, syntax, admission, or
//! deserialization failure without exposing redacted input by default.
//!
//! # Values and trees
//!
//! [`value::JsonValueSeed`] accounts decoded values when only Serde events are
//! available. [`tree::JsonTreeReader`] and [`tree::JsonTreeMutator`] process a
//! materialized value without Rust recursion. [`tree::JsonTreeBudgetTracker`]
//! offers reusable whole-tree accounting; traversal failures use
//! [`tree::JsonTreeProcessError`]. Mutable processing is incremental and does
//! not roll back mutations already accepted before an error.

#![deny(missing_docs)]

mod internal;
pub mod lenient;
pub mod text;
pub mod tree;
pub mod value;

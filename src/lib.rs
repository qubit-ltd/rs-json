// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provides the public API for the `qubit-json` crate.
//!
//! The crate provides resource-aware infrastructure for lenient input,
//! strict text codecs, decoded values, and materialized JSON trees.
//!
//! * [`lenient`] normalizes narrowly defined text noise before direct Serde
//!   deserialization. Its `decode_with_session` path charges raw input,
//!   normalized input, and decoded-value resources cumulatively.
//! * [`text`] strictly decodes and encodes JSON using caller-managed sessions
//!   from [`qubit_budget::json`].
//! * [`value`] constructs [`serde_json::Value`] trees with decoded-value
//!   accounting.
//! * [`tree`] traverses or mutates materialized values without Rust recursion.
//!   Mutable traversal is incremental rather than transactional.
//!
//! # Quick start
//!
//! ```rust
//! use qubit_json::lenient::{JsonDecodeOptions, LenientJsonDecoder};
//!
//! let decoder = LenientJsonDecoder::new(
//!     JsonDecodeOptions::default().with_max_input_bytes(Some(1024)),
//! );
//! let value = decoder.decode_value("```json\n{\"ok\":true}\n```")?;
//!
//! assert_eq!(value["ok"], true);
//! # Ok::<(), qubit_json::lenient::LenientJsonDecodeError>(())
//! ```
//!
//! # Error privacy
//!
//! Errors are redacted by default: input-derived serde messages and values are
//! excluded from the message, debug representation, and standard error source.
//! Safe structural metadata, including parser locations and UTF-8 failure
//! offsets, remains available. Detailed diagnostics must be enabled explicitly
//! and may expose input values.
//!
//! ```rust
//! use qubit_json::lenient::{
//!     ErrorPrivacyPolicy,
//!     JsonDecodeOptions,
//!     LenientJsonDecoder,
//! };
//!
//! let redacted = LenientJsonDecoder::default()
//!     .decode::<u64>(r#""TOP_SECRET""#)
//!     .expect_err("a JSON string cannot deserialize into u64");
//! assert_eq!(redacted.privacy_policy(), ErrorPrivacyPolicy::Redacted);
//! assert!(!redacted.to_string().contains("TOP_SECRET"));
//! assert!(std::error::Error::source(&redacted).is_none());
//!
//! let detailed = LenientJsonDecoder::new(
//!     JsonDecodeOptions::default()
//!         .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
//! )
//! .decode::<u64>(r#""TOP_SECRET""#)
//! .expect_err("a JSON string cannot deserialize into u64");
//! assert_eq!(detailed.privacy_policy(), ErrorPrivacyPolicy::Detailed);
//! assert!(std::error::Error::source(&detailed).is_some());
//! ```
//!
//! # Cumulative lenient decoding
//!
//! `decode_with_session` first charges raw bytes during normalization, then
//! normalized bytes, then uses a lexical preflight to charge JSON value
//! resources before directly deserializing `T`. It does not build an
//! intermediate [`serde_json::Value`], so target-specific Serde behavior is
//! preserved. Raw and normalized input charges remain in the session after an
//! error, while staged value charges commit only after complete
//! deserialization.
//!
//! ```rust
//! use qubit_budget::json::{JsonDecodeLimits, JsonDecodeSession};
//! use qubit_json::lenient::LenientJsonDecoder;
//!
//! let limits = JsonDecodeLimits::empty()
//!     .with_max_input_bytes(32)
//!     .with_max_normalized_input_bytes(32)
//!     .with_max_nodes(4)
//!     .with_max_map_entries(1)
//!     .with_max_key_bytes(2)
//!     .with_max_payload_bytes(4);
//! let mut session = JsonDecodeSession::owned(limits);
//! let decoder = LenientJsonDecoder::default();
//!
//! let value: serde_json::Value = decoder
//!     .decode_with_session("```json\n{\"ok\":true}\n```", &mut session)?;
//! assert_eq!(value["ok"], true);
//! assert_eq!(session.value_budget().used_nodes(), Some(2));
//! # Ok::<(), qubit_json::lenient::LenientJsonDecodeError>(())
//! ```
//!
//! # Budgeted value construction
//!
//! [`value::AccountingJsonValueSeed`] is the public seed for callers that need
//! a materialized [`serde_json::Value`] and only have access to Serde's decoded
//! value events.
//!
//! ```rust
//! use qubit_budget::json::{JsonValueBudget, JsonValueLimits};
//! use qubit_json::value::AccountingJsonValueSeed;
//! use serde::de::DeserializeSeed;
//! use serde_json::Deserializer;
//!
//! let mut budget = JsonValueBudget::new(
//!     JsonValueLimits::empty().with_max_nodes(3),
//! );
//! let mut deserializer = Deserializer::from_slice(br#"{"key":[true]}"#);
//! let mut transaction = budget.transaction();
//! let value = AccountingJsonValueSeed::new(&mut transaction)
//!     .deserialize(&mut deserializer)?;
//! transaction.commit();
//!
//! assert_eq!(value["key"][0], true);
//! assert_eq!(budget.used_nodes(), Some(3));
//! # Ok::<(), serde_json::Error>(())
//! ```
//!
//! # Must-use configuration values
//!
//! ```compile_fail
//! #![deny(unused_must_use)]
//! qubit_json::lenient::JsonDecodeOptions::default();
//! ```
//!
//! ```compile_fail
//! #![deny(unused_must_use)]
//! qubit_json::lenient::LenientJsonDecoder::default();
//! ```

#![deny(missing_docs)]

mod budget;
mod error;
mod internal;
mod json_top_level_kind;
pub mod lenient;
mod lenient_json_decoder;
mod options;
pub mod text;
pub mod tree;
pub mod value;

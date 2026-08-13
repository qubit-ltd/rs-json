// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provides the public API for the `qubit-json` crate.
//!
//! The crate provides resource-aware infrastructure for JSON trees and text.
//!
//! * [`tree`] traverses materialized [`serde_json::Value`] trees without
//!   recursion.
//! * [`text`] strictly decodes and encodes JSON using sessions from
//!   [`qubit_budget::json`].
//! * [`lenient`] normalizes non-standard JSON text before decoding.
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

pub use budget::BudgetedJsonValueSeed;
pub use budget::JsonSerdeError;
pub use budget::JsonSyntaxError;
pub use budget::JsonSyntaxErrorReason;
pub use budget::account_value;
pub use budget::decode_slice;
pub use budget::decode_slice_seed;
pub use budget::encode_to_vec;
pub use budget::encode_to_writer;
pub use error::ErrorPrivacyPolicy;
pub use error::JsonDecodeError;
pub use error::JsonDecodeErrorKind;
pub use error::JsonDecodeStage;
pub use json_top_level_kind::JsonTopLevelKind;
pub use lenient_json_decoder::LenientJsonDecoder;
pub use options::JsonDecodeOptions;
pub use options::MarkdownFenceClosing;
pub use options::MarkdownFencePolicy;
pub use qubit_budget::json::JsonDecodeLimits;
pub use qubit_budget::json::JsonDecodeSession;
pub use qubit_budget::json::JsonEncodeLimits;
pub use qubit_budget::json::JsonEncodeSession;
pub use qubit_budget::json::JsonResource;
pub use qubit_budget::json::JsonValueBudget;
pub use qubit_budget::json::JsonValueLimits;

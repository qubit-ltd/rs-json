// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provides the public API for the `qubit-json` crate.
//!
//! The crate exposes a lenient JSON decoder and the related option and error
//! types needed to normalize and deserialize JSON text from
//! non-fully-trusted sources.
//!
//! # Quick start
//!
//! ```rust
//! use qubit_json::{JsonDecodeOptions, LenientJsonDecoder};
//!
//! let decoder = LenientJsonDecoder::new(
//!     JsonDecodeOptions::default().with_max_input_bytes(Some(1024)),
//! );
//! let value = decoder.decode_value("```json\n{\"ok\":true}\n```")?;
//!
//! assert_eq!(value["ok"], true);
//! # Ok::<(), qubit_json::JsonDecodeError>(())
//! ```
//!
//! # Error privacy
//!
//! Errors are redacted by default: input-derived serde details are discarded
//! from the message, debug representation, and standard error source. Detailed
//! diagnostics must be enabled explicitly and may expose input values.
//!
//! ```rust
//! use qubit_json::{
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
//! qubit_json::JsonDecodeOptions::default();
//! ```
//!
//! ```compile_fail
//! #![deny(unused_must_use)]
//! qubit_json::LenientJsonDecoder::default();
//! ```

#![deny(missing_docs)]

mod error_privacy_policy;
mod internal;
mod json_decode_error;
mod json_decode_error_kind;
mod json_decode_options;
mod json_decode_stage;
mod json_top_level_kind;
mod lenient_json_decoder;
mod markdown_fence_closing;
mod markdown_fence_policy;

pub use error_privacy_policy::ErrorPrivacyPolicy;
pub use json_decode_error::JsonDecodeError;
pub use json_decode_error_kind::JsonDecodeErrorKind;
pub use json_decode_options::JsonDecodeOptions;
pub use json_decode_stage::JsonDecodeStage;
pub use json_top_level_kind::JsonTopLevelKind;
pub use lenient_json_decoder::LenientJsonDecoder;
pub use markdown_fence_closing::MarkdownFenceClosing;
pub use markdown_fence_policy::MarkdownFencePolicy;

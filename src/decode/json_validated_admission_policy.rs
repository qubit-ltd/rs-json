// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Admission ordering for validated JSON decoding.

/// Selects how validated decoding admits JSON tokens before checking what
/// follows them.
///
/// `Immediate` preserves the existing admission order. `TokenFirst` is an
/// opt-in policy for validating a complete token before reporting a following
/// delimiter error.
///
/// # Examples
///
/// Both policies reject malformed JSON. `Immediate` reports the error while
/// parsing the value; `TokenFirst` first admits the complete `true` token and
/// reports the invalid following byte while checking the object separator.
/// The [validated decoding guide](https://github.com/qubit-ltd/rs-json/blob/main/doc/user_guide.md#inspect-original-number-tokens)
/// provides further context. The linked guide tracks the repository's main
/// branch and is additional reading, not a version-specific contract.
/// See also [`JsonValidatedDecodeOptions`](crate::decode::JsonValidatedDecodeOptions).
///
/// ```
/// use std::convert::Infallible;
///
/// use qubit_json::decode::JsonDecodeContext;
/// use qubit_json::decode::JsonDecodeErrorKind;
/// use qubit_json::decode::JsonDecoder;
/// use qubit_json::decode::JsonNumberPolicy;
/// use qubit_json::decode::JsonValidatedAdmissionPolicy;
/// use qubit_json::decode::JsonValidatedDecodeError;
/// use qubit_json::decode::JsonValidatedDecodeOptions;
///
/// for (policy, expected) in [
///     (JsonValidatedAdmissionPolicy::Immediate, JsonDecodeContext::Value),
///     (JsonValidatedAdmissionPolicy::TokenFirst, JsonDecodeContext::ObjectSeparator),
/// ] {
///     let options = JsonValidatedDecodeOptions::builder()
///         .admission_policy(policy)
///         .build();
///     let error = JsonDecoder::unlimited()
///         .decode_unique_validated_str(r#"{"a":truex}"#, &options, |_, _| {
///             Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
///         })
///         .expect_err("both policies still require strict JSON");
///     match error {
///         JsonValidatedDecodeError::Decode { error, context } => {
///             assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
///             assert_eq!(context, Some(expected));
///             assert_eq!(error.syntax_error().unwrap().context(), context);
///         }
///         other => panic!("unexpected error: {other:?}"),
///     }
/// }
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum JsonValidatedAdmissionPolicy {
    /// Uses the existing immediate syntax-checking order.
    #[default]
    Immediate,
    /// Admits a complete token before validating its following delimiter.
    TokenFirst,
}

// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Number representations accepted by JSON decoding.

/// Controls lexical admission of syntactically valid JSON number tokens.
///
/// Typed decoding still lets Serde construct the requested target type, so its
/// numeric representation can depend on that type and the active Serde
/// features. Validated decoding constructs its owned value according to this
/// model and the selected number policy.
///
/// # Examples
///
/// The default model rejects an integer above `u64::MAX`. The finite fallback
/// admits it as an `f64`, which rounds this value and loses integer precision.
/// This example uses a controlled input and an unlimited decoder for
/// illustration; use finite limits for untrusted input. Typed decoding still
/// lets the target type and active features determine its representation. Use
/// a string or a domain-specific format when exact large integers are needed.
/// See [`JsonValidatedDecodeOptions`](crate::decode::JsonValidatedDecodeOptions)
/// and the [JSON number contract](https://github.com/qubit-ltd/rs-json/blob/main/doc/number_contract.md).
///
/// ```
/// use std::convert::Infallible;
///
/// use qubit_json::decode::JsonDecodeErrorKind;
/// use qubit_json::decode::JsonDecodeNumberModel;
/// use qubit_json::decode::JsonDecoder;
/// use qubit_json::decode::JsonNumberPolicy;
/// use qubit_json::decode::JsonValidatedDecodeError;
/// use qubit_json::decode::JsonValidatedDecodeOptions;
///
/// let input = "18446744073709551617";
/// let strict = JsonValidatedDecodeOptions::builder().build();
/// let error = JsonDecoder::unlimited()
///     .decode_unique_validated_str(input, &strict, |_, _| {
///         Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
///     })
///     .expect_err("the default model rejects integers above u64::MAX");
/// match error {
///     JsonValidatedDecodeError::Decode { error, .. } => {
///         assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
///     }
///     other => panic!("unexpected error: {other:?}"),
/// }
///
/// let fallback = JsonValidatedDecodeOptions::builder()
///     .number_model(JsonDecodeNumberModel::Integer64WithFiniteFallback)
///     .build();
/// let owner = JsonDecoder::unlimited()
///     .decode_unique_validated_str(input, &fallback, |_, _| {
///         Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
///     })
///     .expect("finite fallback admits the token with rounding");
/// assert!(owner.as_value().is_f64());
/// assert_eq!(owner.as_value().as_f64(), Some(18446744073709551616_f64));
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum JsonDecodeNumberModel {
    /// Accept signed and unsigned 64-bit integers and finite floating values.
    #[default]
    Integer64,
    /// Also admit out-of-range integer tokens when they convert to finite
    /// `f64` values.
    ///
    /// Validated decoding with `JsonNumberPolicy::Preserve` materializes these
    /// tokens as finite `f64`; typed decoding leaves their representation to
    /// Serde for the requested target type.
    Integer64WithFiniteFallback,
}
